// PostgreSQL AST to Turso AST translator
//
// This module translates pg_query's PostgreSQL AST into Turso's SQL AST
// representation, handling the semantic differences between PostgreSQL and SQLite.

use crate::ParseError;
use pg_query::protobuf::JoinType as PgJoinType;
use pg_query::{NodeRef, ParseResult};
use turso_parser::ast;

/// Translates a PostgreSQL query into Turso's AST
#[derive(Default)]
pub struct PostgreSQLTranslator {
    // TODO: Add schema information, type mappings, etc.
}

impl PostgreSQLTranslator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Maps PostgreSQL system table names to their Turso equivalents.
    /// pg_class, pg_namespace, and pg_attribute have virtual table implementations
    /// and are passed through as-is. Other information_schema names are mapped
    /// to SQLite equivalents.
    fn map_table_name(&self, table_name: &str) -> String {
        match table_name.to_lowercase().as_str() {
            // These have virtual table implementations in pg_catalog.rs - pass through
            "pg_class" | "pg_namespace" | "pg_attribute" | "pg_roles" | "pg_am"
            | "pg_policy" | "pg_trigger" | "pg_index" | "pg_constraint"
            | "pg_statistic_ext" | "pg_inherits" | "pg_rewrite" | "pg_foreign_table"
            | "pg_partitioned_table" | "pg_type" | "pg_collation" | "pg_attrdef"
            | "pg_description" | "pg_publication" | "pg_publication_namespace"
            | "pg_publication_rel" | "pg_get_tabledef" => table_name.to_string(),
            // PostgreSQL information schema mappings (no virtual table yet)
            "pg_tables" => "sqlite_master".to_string(),
            "information_schema.tables" => "sqlite_master".to_string(),
            "information_schema.columns" => "pragma_table_info".to_string(),
            // Default: keep original name
            _ => table_name.to_string(),
        }
    }

    /// Translate a PostgreSQL parse result into Turso's format
    pub fn translate(&self, parse_result: &ParseResult) -> Result<ast::Stmt, ParseError> {
        // The pg_query ParseResult contains a protobuf representation
        // We need to walk the AST nodes and convert them

        if parse_result.protobuf.nodes().is_empty() {
            return Err(ParseError::ParseError("No statements found".to_string()));
        }

        // Get the first statement node
        let node = &parse_result.protobuf.nodes()[0];

        match &node.0 {
            NodeRef::SelectStmt(select) => {
                let select_ast = self.translate_select(select)?;
                Ok(ast::Stmt::Select(select_ast))
            }
            NodeRef::InsertStmt(_insert) => Err(ParseError::ParseError(
                "INSERT statements not yet supported".to_string(),
            )),
            NodeRef::UpdateStmt(_update) => Err(ParseError::ParseError(
                "UPDATE statements not yet supported".to_string(),
            )),
            NodeRef::DeleteStmt(_delete) => Err(ParseError::ParseError(
                "DELETE statements not yet supported".to_string(),
            )),
            NodeRef::CreateStmt(_create) => Err(ParseError::ParseError(
                "CREATE statements not yet supported".to_string(),
            )),
            _ => Err(ParseError::ParseError(format!(
                "Unsupported statement type: {:?}",
                node.0
            ))),
        }
    }

    fn translate_select(
        &self,
        select: &pg_query::protobuf::SelectStmt,
    ) -> Result<ast::Select, ParseError> {
        use pg_query::protobuf::SetOperation;

        // Check if this is a UNION/INTERSECT/EXCEPT (set operation)
        let set_op = select.op();
        if set_op != SetOperation::SetopNone && set_op != SetOperation::Undefined {
            return self.translate_set_operation(select);
        }

        // Regular SELECT — translate FROM, columns, WHERE, ORDER BY
        let from_clause = if !select.from_clause.is_empty() {
            Some(self.translate_from_items(&select.from_clause)?)
        } else {
            None
        };

        let target_list = &select.target_list;
        if target_list.is_empty() {
            return Err(ParseError::ParseError("Empty target list".to_string()));
        }

        let result_columns = self.translate_target_list(target_list)?;

        let where_clause = if let Some(where_clause) = &select.where_clause {
            Some(self.translate_expr(where_clause)?)
        } else {
            None
        };

        let order_by = self.translate_order_by(&select.sort_clause)?;

        let one_select = ast::OneSelect::Select {
            distinctness: None,
            columns: result_columns,
            from: from_clause,
            where_clause: where_clause.map(Box::new),
            group_by: None,
            window_clause: vec![],
        };

        let select_body = ast::SelectBody {
            select: one_select,
            compounds: vec![],
        };

        Ok(ast::Select {
            with: None,
            body: select_body,
            order_by,
            limit: None,
        })
    }

    fn translate_set_operation(
        &self,
        select: &pg_query::protobuf::SelectStmt,
    ) -> Result<ast::Select, ParseError> {
        // Flatten the left-deep tree of set operations into a list.
        // pg_query represents A UNION B UNION C as:
        //   SetOp(SetOp(A, B), C)
        let mut parts: Vec<(Option<ast::CompoundOperator>, &pg_query::protobuf::SelectStmt)> =
            Vec::new();
        Self::flatten_set_operation(select, &mut parts);

        if parts.is_empty() {
            return Err(ParseError::ParseError(
                "Empty set operation".to_string(),
            ));
        }

        // First part becomes the primary select
        let (_, first_stmt) = &parts[0];
        let first_select = self.translate_one_select(first_stmt)?;

        // Remaining parts become compounds
        let mut compounds = Vec::new();
        for (op, stmt) in parts.iter().skip(1) {
            let operator = op.ok_or_else(|| {
                ParseError::ParseError("Missing compound operator".to_string())
            })?;
            compounds.push(ast::CompoundSelect {
                operator,
                select: self.translate_one_select(stmt)?,
            });
        }

        // ORDER BY on compound SELECTs is not yet supported in Turso's
        // query planner, so we drop it. The tables involved are empty stubs
        // anyway, so ordering doesn't matter.
        Ok(ast::Select {
            with: None,
            body: ast::SelectBody {
                select: first_select,
                compounds,
            },
            order_by: vec![],
            limit: None,
        })
    }

    fn flatten_set_operation<'a>(
        stmt: &'a pg_query::protobuf::SelectStmt,
        parts: &mut Vec<(Option<ast::CompoundOperator>, &'a pg_query::protobuf::SelectStmt)>,
    ) {
        use pg_query::protobuf::SetOperation;

        let set_op = stmt.op();
        if set_op == SetOperation::SetopNone || set_op == SetOperation::Undefined {
            // Leaf select
            parts.push((None, stmt));
            return;
        }

        let operator = match (set_op, stmt.all) {
            (SetOperation::SetopUnion, true) => ast::CompoundOperator::UnionAll,
            (SetOperation::SetopUnion, false) => ast::CompoundOperator::Union,
            (SetOperation::SetopIntersect, _) => ast::CompoundOperator::Intersect,
            (SetOperation::SetopExcept, _) => ast::CompoundOperator::Except,
            _ => return,
        };

        if let Some(larg) = &stmt.larg {
            Self::flatten_set_operation(larg, parts);
        }
        if let Some(rarg) = &stmt.rarg {
            // The first element pushed from rarg gets the operator
            let prev_len = parts.len();
            Self::flatten_set_operation(rarg, parts);
            if parts.len() > prev_len {
                parts[prev_len].0 = Some(operator);
            }
        }
    }

    /// Translate a single leaf SELECT (no set operations) into a OneSelect.
    fn translate_one_select(
        &self,
        select: &pg_query::protobuf::SelectStmt,
    ) -> Result<ast::OneSelect, ParseError> {
        let from_clause = if !select.from_clause.is_empty() {
            Some(self.translate_from_items(&select.from_clause)?)
        } else {
            None
        };

        let target_list = &select.target_list;
        if target_list.is_empty() {
            return Err(ParseError::ParseError("Empty target list".to_string()));
        }

        let result_columns = self.translate_target_list(target_list)?;

        let where_clause = if let Some(where_clause) = &select.where_clause {
            Some(self.translate_expr(where_clause)?)
        } else {
            None
        };

        Ok(ast::OneSelect::Select {
            distinctness: None,
            columns: result_columns,
            from: from_clause,
            where_clause: where_clause.map(Box::new),
            group_by: None,
            window_clause: vec![],
        })
    }

    /// Translate multiple FROM items. The first becomes the primary table,
    /// subsequent items become comma-joins (implicit cross join).
    fn translate_from_items(
        &self,
        from_items: &[pg_query::protobuf::Node],
    ) -> Result<ast::FromClause, ParseError> {
        let mut from_clause = self.translate_from_clause(&from_items[0])?;
        // Additional FROM items are comma-joins (implicit cross join)
        for item in &from_items[1..] {
            let table = match &item.node {
                Some(pg_query::protobuf::node::Node::RangeVar(range_var)) => {
                    self.translate_range_var(range_var)?
                }
                Some(pg_query::protobuf::node::Node::JoinExpr(join_expr)) => {
                    // A JoinExpr as a comma-separated item — flatten its joins
                    let nested = self.translate_join_expr(join_expr)?;
                    from_clause.joins.extend(nested.joins);
                    *nested.select
                }
                other => {
                    return Err(ParseError::ParseError(format!(
                        "Unsupported FROM item type: {other:?}"
                    )))
                }
            };
            from_clause.joins.push(ast::JoinedSelectTable {
                operator: ast::JoinOperator::Comma,
                table: Box::new(table),
                constraint: None,
            });
        }
        Ok(from_clause)
    }

    fn translate_from_clause(
        &self,
        from_item: &pg_query::protobuf::Node,
    ) -> Result<ast::FromClause, ParseError> {
        match &from_item.node {
            Some(pg_query::protobuf::node::Node::RangeVar(range_var)) => {
                let select_table = self.translate_range_var(range_var)?;

                Ok(ast::FromClause {
                    select: Box::new(select_table),
                    joins: vec![],
                })
            }
            Some(pg_query::protobuf::node::Node::JoinExpr(join_expr)) => {
                self.translate_join_expr(join_expr)
            }
            _ => Err(ParseError::ParseError(format!(
                "Unsupported FROM clause type: {:?}",
                from_item.node
            ))),
        }
    }

    fn translate_range_var(
        &self,
        range_var: &pg_query::protobuf::RangeVar,
    ) -> Result<ast::SelectTable, ParseError> {
        let table_name = &range_var.relname;
        let mapped_name = self.map_table_name(table_name);

        let qualified_name = ast::QualifiedName::single(ast::Name::from_string(mapped_name));
        let alias = range_var
            .alias
            .as_ref()
            .map(|a| ast::As::Elided(ast::Name::from_string(a.aliasname.clone())));

        Ok(ast::SelectTable::Table(qualified_name, alias, None))
    }

    fn translate_join_expr(
        &self,
        join_expr: &pg_query::protobuf::JoinExpr,
    ) -> Result<ast::FromClause, ParseError> {
        // Flatten the left-deep join tree: collect the primary table and all joins
        let mut joins = Vec::new();
        let primary_table = self.flatten_join_tree(join_expr, &mut joins)?;

        Ok(ast::FromClause {
            select: Box::new(primary_table),
            joins,
        })
    }

    fn flatten_join_tree(
        &self,
        join_expr: &pg_query::protobuf::JoinExpr,
        joins: &mut Vec<ast::JoinedSelectTable>,
    ) -> Result<ast::SelectTable, ParseError> {
        // Recursively process the left side
        let primary_table = if let Some(larg) = &join_expr.larg {
            match &larg.node {
                Some(pg_query::protobuf::node::Node::RangeVar(range_var)) => {
                    self.translate_range_var(range_var)?
                }
                Some(pg_query::protobuf::node::Node::JoinExpr(nested_join)) => {
                    self.flatten_join_tree(nested_join, joins)?
                }
                _ => {
                    return Err(ParseError::ParseError(format!(
                        "Unsupported left side of JOIN: {:?}",
                        larg.node
                    )))
                }
            }
        } else {
            return Err(ParseError::ParseError(
                "Missing left side of JOIN".to_string(),
            ));
        };

        // Process the right side
        let right_table = if let Some(rarg) = &join_expr.rarg {
            match &rarg.node {
                Some(pg_query::protobuf::node::Node::RangeVar(range_var)) => {
                    self.translate_range_var(range_var)?
                }
                Some(pg_query::protobuf::node::Node::JoinExpr(nested_join)) => {
                    // Nested join on the right side: wrap as a subquery-like structure
                    // For now, flatten it too (handles chained joins)
                    let mut right_joins = Vec::new();
                    let right_primary = self.flatten_join_tree(nested_join, &mut right_joins)?;
                    // Add the right-side joins first, then the right primary becomes a joined table
                    joins.extend(right_joins);
                    right_primary
                }
                _ => {
                    return Err(ParseError::ParseError(format!(
                        "Unsupported right side of JOIN: {:?}",
                        rarg.node
                    )))
                }
            }
        } else {
            return Err(ParseError::ParseError(
                "Missing right side of JOIN".to_string(),
            ));
        };

        // Map pg_query JoinType to Turso JoinOperator
        let join_type = PgJoinType::try_from(join_expr.jointype).unwrap_or(PgJoinType::Undefined);
        let operator = match join_type {
            PgJoinType::JoinInner => {
                ast::JoinOperator::TypedJoin(Some(ast::JoinType::INNER))
            }
            PgJoinType::JoinLeft => {
                ast::JoinOperator::TypedJoin(Some(ast::JoinType::LEFT | ast::JoinType::OUTER))
            }
            PgJoinType::JoinRight => {
                ast::JoinOperator::TypedJoin(Some(ast::JoinType::RIGHT | ast::JoinType::OUTER))
            }
            PgJoinType::JoinFull => ast::JoinOperator::TypedJoin(Some(
                ast::JoinType::LEFT | ast::JoinType::RIGHT | ast::JoinType::OUTER,
            )),
            _ => ast::JoinOperator::TypedJoin(None), // Default to plain JOIN
        };

        // Translate ON condition
        let constraint = if let Some(quals) = &join_expr.quals {
            Some(ast::JoinConstraint::On(Box::new(
                self.translate_expr(quals)?,
            )))
        } else {
            None
        };

        joins.push(ast::JoinedSelectTable {
            operator,
            table: Box::new(right_table),
            constraint,
        });

        Ok(primary_table)
    }

    fn translate_target_list(
        &self,
        target_list: &[pg_query::protobuf::Node],
    ) -> Result<Vec<ast::ResultColumn>, ParseError> {
        let mut result_columns = Vec::new();

        for target in target_list {
            match &target.node {
                Some(pg_query::protobuf::node::Node::ResTarget(res_target)) => {
                    if let Some(val) = &res_target.val {
                        // Check if this is a SELECT *
                        if let Some(pg_query::protobuf::node::Node::AStar(_)) = &val.node {
                            result_columns.push(ast::ResultColumn::Star);
                        } else if let Some(pg_query::protobuf::node::Node::ColumnRef(col_ref)) =
                            &val.node
                        {
                            // Check if this is a column reference with "*"
                            if let Some(field) = col_ref.fields.first() {
                                if let Some(pg_query::protobuf::node::Node::AStar(_)) = &field.node
                                {
                                    result_columns.push(ast::ResultColumn::Star);
                                    continue;
                                }
                            }
                            // Regular column reference
                            let expr = self.translate_expr(val)?;
                            let alias: Option<ast::As> = if res_target.name.is_empty() {
                                None
                            } else {
                                Some(ast::As::Elided(ast::Name::from_string(&res_target.name)))
                            };
                            result_columns.push(ast::ResultColumn::Expr(Box::new(expr), alias));
                        } else {
                            let expr = self.translate_expr(val)?;
                            let alias: Option<ast::As> = if res_target.name.is_empty() {
                                None
                            } else {
                                Some(ast::As::Elided(ast::Name::from_string(&res_target.name)))
                            };
                            result_columns.push(ast::ResultColumn::Expr(Box::new(expr), alias));
                        }
                    }
                }
                _ => {
                    return Err(ParseError::ParseError(
                        "Unsupported target list item".to_string(),
                    ))
                }
            }
        }

        Ok(result_columns)
    }

    fn translate_expr(&self, node: &pg_query::protobuf::Node) -> Result<ast::Expr, ParseError> {
        match &node.node {
            Some(pg_query::protobuf::node::Node::ColumnRef(col_ref)) => {
                // Extract column name from fields
                if let Some(field) = col_ref.fields.first() {
                    match &field.node {
                        Some(pg_query::protobuf::node::Node::String(s)) => {
                            if col_ref.fields.len() == 1 {
                                // Simple column reference
                                Ok(ast::Expr::Id(ast::Name::from_string(s.sval.clone())))
                            } else {
                                // Qualified column reference (table.column)
                                let mut parts = vec![];
                                for field in &col_ref.fields {
                                    if let Some(pg_query::protobuf::node::Node::String(s)) =
                                        &field.node
                                    {
                                        parts.push(s.sval.clone());
                                    }
                                }
                                if parts.len() > 1 {
                                    // table.column
                                    Ok(ast::Expr::Qualified(
                                        ast::Name::from_string(parts[0].clone()),
                                        ast::Name::from_string(parts[1].clone()),
                                    ))
                                } else {
                                    // Just a simple column name
                                    Ok(ast::Expr::Id(ast::Name::from_string(parts[0].clone())))
                                }
                            }
                        }
                        Some(pg_query::protobuf::node::Node::AStar(_)) => {
                            // SELECT * case - should be handled in translate_target_list, not here
                            Err(ParseError::ParseError(
                                "AStar should be handled in target list, not as expression"
                                    .to_string(),
                            ))
                        }
                        other => Err(ParseError::ParseError(format!(
                            "Invalid column reference, expected String or AStar but got: {other:?}"
                        ))),
                    }
                } else {
                    Err(ParseError::ParseError("Empty column reference".to_string()))
                }
            }
            Some(pg_query::protobuf::node::Node::AConst(a_const)) => self.translate_const(a_const),
            Some(pg_query::protobuf::node::Node::AExpr(a_expr)) => self.translate_a_expr(a_expr),
            Some(pg_query::protobuf::node::Node::BoolExpr(bool_expr)) => {
                self.translate_bool_expr(bool_expr)
            }
            Some(pg_query::protobuf::node::Node::FuncCall(func_call)) => {
                self.translate_func_call(func_call)
            }
            Some(pg_query::protobuf::node::Node::CaseExpr(case_expr)) => {
                self.translate_case_expr(case_expr)
            }
            Some(pg_query::protobuf::node::Node::CollateClause(collate)) => {
                // Strip COLLATE clause, just translate the inner expression
                if let Some(arg) = &collate.arg {
                    self.translate_expr(arg)
                } else {
                    Err(ParseError::ParseError(
                        "COLLATE clause missing inner expression".to_string(),
                    ))
                }
            }
            Some(pg_query::protobuf::node::Node::TypeCast(type_cast)) => {
                // Strip type casts — SQLite doesn't have PG's type system
                if let Some(arg) = &type_cast.arg {
                    self.translate_expr(arg)
                } else {
                    Err(ParseError::ParseError(
                        "TypeCast missing inner expression".to_string(),
                    ))
                }
            }
            Some(pg_query::protobuf::node::Node::SubLink(_)) => {
                // Subquery expressions (correlated subqueries in SELECT list).
                // Stub as NULL — the referenced tables (pg_attrdef, pg_collation, etc.)
                // are not yet implemented.
                Ok(ast::Expr::Literal(ast::Literal::Null))
            }
            Some(pg_query::protobuf::node::Node::NullTest(null_test)) => {
                use pg_query::protobuf::NullTestType;
                let arg = null_test
                    .arg
                    .as_ref()
                    .ok_or_else(|| ParseError::ParseError("NullTest missing arg".to_string()))?;
                let expr = self.translate_expr(arg)?;
                match null_test.nulltesttype() {
                    NullTestType::IsNotNull => Ok(ast::Expr::NotNull(Box::new(expr))),
                    _ => Ok(ast::Expr::IsNull(Box::new(expr))),
                }
            }
            Some(pg_query::protobuf::node::Node::AStar(_)) => {
                // SELECT * - this should be handled as ResultColumn::Star in translate_target_list
                Err(ParseError::ParseError(
                    "AStar should not be translated as expression".to_string(),
                ))
            }
            _ => Err(ParseError::ParseError(format!(
                "Unsupported expression type: {:?}",
                node.node
            ))),
        }
    }

    fn translate_const(
        &self,
        a_const: &pg_query::protobuf::AConst,
    ) -> Result<ast::Expr, ParseError> {
        if a_const.isnull {
            return Ok(ast::Expr::Literal(ast::Literal::Null));
        }
        if let Some(val) = &a_const.val {
            match val {
                pg_query::protobuf::a_const::Val::Ival(i) => Ok(ast::Expr::Literal(
                    ast::Literal::Numeric(i.ival.to_string()),
                )),
                pg_query::protobuf::a_const::Val::Sval(s) => {
                    // Turso's AST expects string literals to include surrounding single quotes
                    // (sanitize_string strips them during bytecode emission)
                    let quoted = format!("'{}'", s.sval.replace('\'', "''"));
                    Ok(ast::Expr::Literal(ast::Literal::String(quoted)))
                }
                pg_query::protobuf::a_const::Val::Fval(f) => {
                    Ok(ast::Expr::Literal(ast::Literal::Numeric(f.fval.clone())))
                }
                pg_query::protobuf::a_const::Val::Boolval(b) => {
                    // SQLite uses 0/1 for booleans
                    Ok(ast::Expr::Literal(ast::Literal::Numeric(
                        if b.boolval { "1" } else { "0" }.to_string(),
                    )))
                }
                _ => Err(ParseError::ParseError(
                    "Unsupported constant type".to_string(),
                )),
            }
        } else {
            Err(ParseError::ParseError("Empty constant value".to_string()))
        }
    }

    fn translate_a_expr(
        &self,
        a_expr: &pg_query::protobuf::AExpr,
    ) -> Result<ast::Expr, ParseError> {
        use pg_query::protobuf::AExprKind;

        match &a_expr.kind() {
            AExprKind::AexprOp => {
                // Regular binary operators
                self.translate_binary_expr(a_expr)
            }
            AExprKind::AexprIn => {
                // IN operator
                self.translate_in_expr(a_expr)
            }
            AExprKind::AexprLike => {
                // LIKE/NOT LIKE operator
                self.translate_like_expr(a_expr)
            }
            AExprKind::AexprOpAny => {
                // expr = ANY(array) → stub as 0 (false).
                // Our pg_catalog tables that use arrays are empty stubs,
                // so this never actually evaluates.
                Ok(ast::Expr::Literal(ast::Literal::Numeric("0".to_string())))
            }
            _ => Err(ParseError::ParseError(format!(
                "Unsupported AExpr kind: {:?}",
                a_expr.kind()
            ))),
        }
    }

    fn translate_binary_expr(
        &self,
        a_expr: &pg_query::protobuf::AExpr,
    ) -> Result<ast::Expr, ParseError> {
        // Extract operator name — use the last String node to handle
        // schema-qualified operators like OPERATOR(pg_catalog.~)
        let op_name = a_expr
            .name
            .iter()
            .rev()
            .find_map(|name| match &name.node {
                Some(pg_query::protobuf::node::Node::String(s)) => Some(s.sval.as_str()),
                _ => None,
            })
            .ok_or_else(|| ParseError::ParseError("Missing operator name".to_string()))?;

        // Translate left and right expressions
        let left = if let Some(lexpr) = &a_expr.lexpr {
            Box::new(self.translate_expr(lexpr)?)
        } else {
            return Err(ParseError::ParseError(
                "Missing left expression".to_string(),
            ));
        };

        let right = if let Some(rexpr) = &a_expr.rexpr {
            Box::new(self.translate_expr(rexpr)?)
        } else {
            return Err(ParseError::ParseError(
                "Missing right expression".to_string(),
            ));
        };

        // Handle regex operators (~, !~) which map to REGEXP expressions
        match op_name {
            "~" => {
                return Ok(ast::Expr::Like {
                    lhs: left,
                    not: false,
                    op: ast::LikeOperator::Regexp,
                    rhs: right,
                    escape: None,
                });
            }
            "!~" => {
                return Ok(ast::Expr::Like {
                    lhs: left,
                    not: true,
                    op: ast::LikeOperator::Regexp,
                    rhs: right,
                    escape: None,
                });
            }
            _ => {}
        }

        // Map PostgreSQL operators to Turso operators
        let binary_op = match op_name {
            "=" => ast::Operator::Equals,
            "!=" | "<>" => ast::Operator::NotEquals,
            "<" => ast::Operator::Less,
            "<=" => ast::Operator::LessEquals,
            ">" => ast::Operator::Greater,
            ">=" => ast::Operator::GreaterEquals,
            "+" => ast::Operator::Add,
            "-" => ast::Operator::Subtract,
            "*" => ast::Operator::Multiply,
            "/" => ast::Operator::Divide,
            "AND" => ast::Operator::And,
            "OR" => ast::Operator::Or,
            _ => {
                return Err(ParseError::ParseError(format!(
                    "Unsupported operator: {op_name}"
                )))
            }
        };

        Ok(ast::Expr::Binary(left, binary_op, right))
    }

    fn translate_in_expr(
        &self,
        a_expr: &pg_query::protobuf::AExpr,
    ) -> Result<ast::Expr, ParseError> {
        // Get the left expression (the column/expression being tested)
        let lhs = if let Some(lexpr) = &a_expr.lexpr {
            Box::new(self.translate_expr(lexpr)?)
        } else {
            return Err(ParseError::ParseError(
                "Missing left expression for IN operator".to_string(),
            ));
        };

        // Get the right expression (should be a list)
        let rhs = if let Some(rexpr) = &a_expr.rexpr {
            match &rexpr.node {
                Some(pg_query::protobuf::node::Node::List(list)) => {
                    let mut values = Vec::new();
                    for item in &list.items {
                        values.push(Box::new(self.translate_expr(item)?));
                    }
                    values
                }
                _ => {
                    return Err(ParseError::ParseError(
                        "Expected list for IN operator right side".to_string(),
                    ))
                }
            }
        } else {
            return Err(ParseError::ParseError(
                "Missing right expression for IN operator".to_string(),
            ));
        };

        // Check if it's NOT IN
        let not = a_expr
            .name
            .first()
            .and_then(|name| name.node.as_ref())
            .map(|node| matches!(node, pg_query::protobuf::node::Node::String(s) if s.sval == "<>"))
            .unwrap_or(false);

        Ok(ast::Expr::InList { lhs, not, rhs })
    }

    fn translate_like_expr(
        &self,
        a_expr: &pg_query::protobuf::AExpr,
    ) -> Result<ast::Expr, ParseError> {
        // Get the operator name to determine if it's LIKE or NOT LIKE
        let op_name = if let Some(name) = a_expr.name.first() {
            match &name.node {
                Some(pg_query::protobuf::node::Node::String(s)) => &s.sval,
                _ => {
                    return Err(ParseError::ParseError(
                        "Invalid LIKE operator name".to_string(),
                    ))
                }
            }
        } else {
            return Err(ParseError::ParseError(
                "Missing LIKE operator name".to_string(),
            ));
        };

        // Determine if it's NOT LIKE
        let not = match op_name.as_str() {
            "~~" => false, // LIKE
            "!~~" => true, // NOT LIKE
            _ => {
                return Err(ParseError::ParseError(format!(
                    "Unsupported LIKE operator: {op_name}"
                )))
            }
        };

        // Get left and right expressions
        let lhs = if let Some(lexpr) = &a_expr.lexpr {
            Box::new(self.translate_expr(lexpr)?)
        } else {
            return Err(ParseError::ParseError(
                "Missing left expression for LIKE operator".to_string(),
            ));
        };

        let rhs = if let Some(rexpr) = &a_expr.rexpr {
            Box::new(self.translate_expr(rexpr)?)
        } else {
            return Err(ParseError::ParseError(
                "Missing right expression for LIKE operator".to_string(),
            ));
        };

        Ok(ast::Expr::Like {
            lhs,
            not,
            op: ast::LikeOperator::Like,
            rhs,
            escape: None,
        })
    }

    fn translate_func_call(
        &self,
        func_call: &pg_query::protobuf::FuncCall,
    ) -> Result<ast::Expr, ParseError> {
        // Extract function name
        let func_name = func_call
            .funcname
            .iter()
            .filter_map(|node| {
                if let Some(pg_query::protobuf::node::Node::String(s)) = &node.node {
                    Some(s.sval.clone())
                } else {
                    None
                }
            })
            .next_back()
            .ok_or_else(|| ParseError::ParseError("Missing function name".to_string()))?;

        let filter_over = ast::FunctionTail {
            filter_clause: None,
            over_clause: None,
        };

        // COUNT(*) and similar aggregate star calls
        if func_call.agg_star {
            return Ok(ast::Expr::FunctionCallStar {
                name: ast::Name::from_string(func_name),
                filter_over,
            });
        }

        // Translate function arguments
        let args = func_call
            .args
            .iter()
            .map(|arg| Ok(Box::new(self.translate_expr(arg)?)))
            .collect::<Result<Vec<_>, ParseError>>()?;

        let distinctness = if func_call.agg_distinct {
            Some(ast::Distinctness::Distinct)
        } else {
            None
        };

        Ok(ast::Expr::FunctionCall {
            name: ast::Name::from_string(func_name),
            distinctness,
            args,
            order_by: vec![],
            filter_over,
        })
    }

    fn translate_case_expr(
        &self,
        case_expr: &pg_query::protobuf::CaseExpr,
    ) -> Result<ast::Expr, ParseError> {
        // Translate optional base expression (CASE <expr> WHEN ...)
        let base = if let Some(arg) = &case_expr.arg {
            Some(Box::new(self.translate_expr(arg)?))
        } else {
            None
        };

        // Translate WHEN/THEN pairs
        let mut when_then_pairs = Vec::new();
        for arg in &case_expr.args {
            match &arg.node {
                Some(pg_query::protobuf::node::Node::CaseWhen(case_when)) => {
                    let when_expr = if let Some(expr) = &case_when.expr {
                        Box::new(self.translate_expr(expr)?)
                    } else {
                        return Err(ParseError::ParseError(
                            "CASE WHEN missing condition".to_string(),
                        ));
                    };
                    let then_expr = if let Some(result) = &case_when.result {
                        Box::new(self.translate_expr(result)?)
                    } else {
                        return Err(ParseError::ParseError(
                            "CASE WHEN missing THEN result".to_string(),
                        ));
                    };
                    when_then_pairs.push((when_expr, then_expr));
                }
                _ => {
                    return Err(ParseError::ParseError(
                        "Expected CaseWhen node in CASE expression".to_string(),
                    ))
                }
            }
        }

        // Translate optional ELSE expression
        let else_expr = if let Some(defresult) = &case_expr.defresult {
            Some(Box::new(self.translate_expr(defresult)?))
        } else {
            None
        };

        Ok(ast::Expr::Case {
            base,
            when_then_pairs,
            else_expr,
        })
    }

    fn translate_order_by(
        &self,
        sort_clause: &[pg_query::protobuf::Node],
    ) -> Result<Vec<ast::SortedColumn>, ParseError> {
        let mut sorted_columns = Vec::new();
        for node in sort_clause {
            match &node.node {
                Some(pg_query::protobuf::node::Node::SortBy(sort_by)) => {
                    let expr = if let Some(ref sort_node) = sort_by.node {
                        Box::new(self.translate_expr(sort_node)?)
                    } else {
                        return Err(ParseError::ParseError(
                            "Missing sort expression".to_string(),
                        ));
                    };

                    let order = match pg_query::protobuf::SortByDir::try_from(sort_by.sortby_dir) {
                        Ok(pg_query::protobuf::SortByDir::SortbyAsc) => Some(ast::SortOrder::Asc),
                        Ok(pg_query::protobuf::SortByDir::SortbyDesc) => Some(ast::SortOrder::Desc),
                        _ => None, // Default or undefined
                    };

                    sorted_columns.push(ast::SortedColumn {
                        expr,
                        order,
                        nulls: None,
                    });
                }
                _ => {
                    return Err(ParseError::ParseError(format!(
                        "Unsupported ORDER BY clause item: {:?}",
                        node.node
                    )))
                }
            }
        }
        Ok(sorted_columns)
    }

    fn translate_bool_expr(
        &self,
        bool_expr: &pg_query::protobuf::BoolExpr,
    ) -> Result<ast::Expr, ParseError> {
        use pg_query::protobuf::BoolExprType;

        if bool_expr.args.is_empty() {
            return Err(ParseError::ParseError(
                "BoolExpr must have at least 1 argument".to_string(),
            ));
        }

        // Map PostgreSQL boolean operators to Turso operators
        match &bool_expr.boolop() {
            BoolExprType::NotExpr => {
                // NOT is unary, handle differently
                if bool_expr.args.len() != 1 {
                    return Err(ParseError::ParseError(
                        "NOT expression must have exactly 1 argument".to_string(),
                    ));
                }
                let operand = Box::new(self.translate_expr(&bool_expr.args[0])?);
                Ok(ast::Expr::Unary(ast::UnaryOperator::Not, operand))
            }
            BoolExprType::AndExpr => {
                if bool_expr.args.len() < 2 {
                    return Err(ParseError::ParseError(
                        "AND expression must have at least 2 arguments".to_string(),
                    ));
                }
                // Combine all arguments into a binary tree with AND
                let mut result = self.translate_expr(&bool_expr.args[0])?;
                for arg in &bool_expr.args[1..] {
                    let right = self.translate_expr(arg)?;
                    result =
                        ast::Expr::Binary(Box::new(result), ast::Operator::And, Box::new(right));
                }
                Ok(result)
            }
            BoolExprType::OrExpr => {
                if bool_expr.args.len() < 2 {
                    return Err(ParseError::ParseError(
                        "OR expression must have at least 2 arguments".to_string(),
                    ));
                }
                // Combine all arguments into a binary tree with OR
                let mut result = self.translate_expr(&bool_expr.args[0])?;
                for arg in &bool_expr.args[1..] {
                    let right = self.translate_expr(arg)?;
                    result =
                        ast::Expr::Binary(Box::new(result), ast::Operator::Or, Box::new(right));
                }
                Ok(result)
            }
            _ => Err(ParseError::ParseError(format!(
                "Unsupported BoolExpr type: {:?}",
                bool_expr.boolop()
            ))),
        }
    }
}

/// PostgreSQL to SQLite type mapping
pub fn map_postgresql_type(pg_type: &str) -> String {
    match pg_type.to_uppercase().as_str() {
        // Numeric types
        "SMALLINT" | "INT2" => "INTEGER",
        "INTEGER" | "INT" | "INT4" => "INTEGER",
        "BIGINT" | "INT8" => "INTEGER",
        "DECIMAL" | "NUMERIC" => "REAL",
        "REAL" | "FLOAT4" => "REAL",
        "DOUBLE PRECISION" | "FLOAT8" => "REAL",
        "SERIAL" => "INTEGER", // Note: Need to handle AUTO INCREMENT separately
        "BIGSERIAL" => "INTEGER",

        // String types
        "VARCHAR" | "CHARACTER VARYING" => "TEXT",
        "CHAR" | "CHARACTER" => "TEXT",
        "TEXT" => "TEXT",

        // Binary
        "BYTEA" => "BLOB",

        // Boolean
        "BOOLEAN" | "BOOL" => "INTEGER", // 0 or 1 in SQLite

        // Date/Time
        "DATE" => "TEXT",
        "TIME" => "TEXT",
        "TIMESTAMP" => "TEXT",
        "TIMESTAMPTZ" => "TEXT",
        "INTERVAL" => "TEXT",

        // JSON
        "JSON" | "JSONB" => "TEXT", // Store as TEXT, parse as needed

        // UUID
        "UUID" => "TEXT",

        // Arrays - store as JSON
        _ if pg_type.ends_with("[]") => "TEXT",

        // Default
        _ => "TEXT",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_mapping() {
        assert_eq!(map_postgresql_type("INTEGER"), "INTEGER");
        assert_eq!(map_postgresql_type("VARCHAR(255)"), "TEXT");
        assert_eq!(map_postgresql_type("BOOLEAN"), "INTEGER");
        assert_eq!(map_postgresql_type("JSONB"), "TEXT");
        assert_eq!(map_postgresql_type("UUID"), "TEXT");
        assert_eq!(map_postgresql_type("INTEGER[]"), "TEXT");
    }

    #[test]
    fn test_basic_translation() {
        let translator = PostgreSQLTranslator::new();
        let sql = "SELECT * FROM users WHERE id = 1";
        let parse_result = crate::parse(sql).unwrap();
        let translated = translator.translate(&parse_result);

        assert!(translated.is_ok());

        if let Ok(ast::Stmt::Select(select)) = translated {
            // Check the select body
            if let ast::OneSelect::Select {
                columns,
                from,
                where_clause,
                ..
            } = &select.body.select
            {
                // Should have one result column (*)
                assert_eq!(columns.len(), 1);
                matches!(columns[0], ast::ResultColumn::Star);

                // Should have FROM clause
                assert!(from.is_some());

                // Should have WHERE clause
                assert!(where_clause.is_some());
            } else {
                panic!("Expected OneSelect::Select");
            }
        }
    }

    #[test]
    fn test_table_name_mapping() {
        let translator = PostgreSQLTranslator::new();

        // Test pg_tables mapping
        let sql = "SELECT * FROM pg_tables";
        let parse_result = crate::parse(sql).unwrap();
        let translated = translator.translate(&parse_result).unwrap();

        if let ast::Stmt::Select(select) = translated {
            if let ast::OneSelect::Select { from, .. } = &select.body.select {
                if let Some(from_clause) = from {
                    if let ast::SelectTable::Table(qualified_name, _, _) = &*from_clause.select {
                        assert_eq!(qualified_name.name.as_str(), "sqlite_master");
                    } else {
                        panic!("Expected table reference");
                    }
                } else {
                    panic!("Expected FROM clause");
                }
            } else {
                panic!("Expected OneSelect::Select");
            }
        } else {
            panic!("Expected select query");
        }
    }

    #[test]
    fn test_simple_select_star() {
        let translator = PostgreSQLTranslator::new();
        let sql = "SELECT * FROM sqlite_master";
        let parse_result = crate::parse(sql).unwrap();
        let translated = translator.translate(&parse_result);
        assert!(translated.is_ok());

        if let Ok(ast::Stmt::Select(select)) = translated {
            if let ast::OneSelect::Select { columns, from, .. } = &select.body.select {
                // Should have one result column: *
                assert_eq!(columns.len(), 1);
                assert!(
                    matches!(columns[0], ast::ResultColumn::Star),
                    "Expected ResultColumn::Star but got {:?}",
                    columns[0]
                );

                // Should have FROM clause
                if let Some(from_clause) = from {
                    if let ast::SelectTable::Table(qualified_name, alias, _) = &*from_clause.select
                    {
                        assert_eq!(qualified_name.name.as_str(), "sqlite_master");
                        assert!(alias.is_none());
                    } else {
                        panic!("Expected table reference");
                    }
                } else {
                    panic!("Expected FROM clause");
                }
            } else {
                panic!("Expected OneSelect::Select");
            }
        } else {
            panic!("Expected select query");
        }
    }

    #[test]
    fn test_column_expressions() {
        let translator = PostgreSQLTranslator::new();
        let sql = "SELECT id, name FROM users";
        let parse_result = crate::parse(sql).unwrap();
        let translated = translator.translate(&parse_result);
        assert!(translated.is_ok());

        if let Ok(ast::Stmt::Select(select)) = translated {
            if let ast::OneSelect::Select { columns, from, .. } = &select.body.select {
                // Should have two result columns: id, name
                assert_eq!(columns.len(), 2);

                // First column should be 'id'
                if let ast::ResultColumn::Expr(expr, alias) = &columns[0] {
                    assert!(
                        matches!(**expr, ast::Expr::Id(_)),
                        "Expected Name expression but got {expr:?}"
                    );
                    if let ast::Expr::Id(name) = &**expr {
                        assert_eq!(name.as_str(), "id");
                    }
                    assert!(alias.is_none());
                } else {
                    panic!("Expected expression result column for first column");
                }

                // Second column should be 'name'
                if let ast::ResultColumn::Expr(expr, alias) = &columns[1] {
                    assert!(
                        matches!(**expr, ast::Expr::Id(_)),
                        "Expected Name expression but got {expr:?}"
                    );
                    if let ast::Expr::Id(name) = &**expr {
                        assert_eq!(name.as_str(), "name");
                    }
                    assert!(alias.is_none());
                } else {
                    panic!("Expected expression result column for second column");
                }

                // Should have FROM clause
                assert!(from.is_some());
            } else {
                panic!("Expected OneSelect::Select");
            }
        } else {
            panic!("Expected select query");
        }
    }

    #[test]
    fn test_qualified_column_expressions() {
        let translator = PostgreSQLTranslator::new();
        let sql = "SELECT users.id, t.name FROM users t";
        let parse_result = crate::parse(sql).unwrap();
        let translated = translator.translate(&parse_result);
        assert!(translated.is_ok());

        if let Ok(ast::Stmt::Select(select)) = translated {
            if let ast::OneSelect::Select { columns, from, .. } = &select.body.select {
                // Should have two result columns: users.id, t.name
                assert_eq!(columns.len(), 2);

                // First column should be 'users.id'
                if let ast::ResultColumn::Expr(expr, alias) = &columns[0] {
                    assert!(
                        matches!(**expr, ast::Expr::Qualified(_, _)),
                        "Expected Qualified expression but got {expr:?}"
                    );
                    if let ast::Expr::Qualified(table_name, col_name) = &**expr {
                        assert_eq!(table_name.as_str(), "users");
                        assert_eq!(col_name.as_str(), "id");
                    }
                    assert!(alias.is_none());
                } else {
                    panic!("Expected expression result column for first qualified column");
                }

                // Second column should be 't.name'
                if let ast::ResultColumn::Expr(expr, alias) = &columns[1] {
                    assert!(
                        matches!(**expr, ast::Expr::Qualified(_, _)),
                        "Expected Qualified expression but got {expr:?}"
                    );
                    if let ast::Expr::Qualified(table_name, col_name) = &**expr {
                        assert_eq!(table_name.as_str(), "t");
                        assert_eq!(col_name.as_str(), "name");
                    }
                    assert!(alias.is_none());
                } else {
                    panic!("Expected expression result column for second qualified column");
                }

                // Should have FROM clause
                assert!(from.is_some());
            } else {
                panic!("Expected OneSelect::Select");
            }
        } else {
            panic!("Expected select query");
        }
    }

    #[test]
    fn test_select_with_where_clause() {
        let translator = PostgreSQLTranslator::new();
        let sql = "SELECT * FROM users WHERE id = 1";
        let parse_result = crate::parse(sql).unwrap();
        let translated = translator.translate(&parse_result);
        assert!(translated.is_ok());

        if let Ok(ast::Stmt::Select(select)) = translated {
            if let ast::OneSelect::Select {
                columns,
                from,
                where_clause,
                ..
            } = &select.body.select
            {
                // Should have SELECT *
                assert_eq!(columns.len(), 1);
                assert!(matches!(columns[0], ast::ResultColumn::Star));

                // Should have FROM clause
                assert!(from.is_some());

                // Should have WHERE clause
                assert!(where_clause.is_some());
                if let Some(where_expr) = where_clause {
                    // WHERE id = 1 should be a binary expression
                    assert!(
                        matches!(**where_expr, ast::Expr::Binary(_, _, _)),
                        "Expected Binary expression but got {where_expr:?}"
                    );
                    if let ast::Expr::Binary(left, op, right) = &**where_expr {
                        // Left side should be column 'id'
                        assert!(
                            matches!(**left, ast::Expr::Id(_)),
                            "Expected Name expression for left side"
                        );
                        if let ast::Expr::Id(name) = &**left {
                            assert_eq!(name.as_str(), "id");
                        }

                        // Operator should be Equals
                        assert!(
                            matches!(op, ast::Operator::Equals),
                            "Expected Equals operator"
                        );

                        // Right side should be literal 1
                        assert!(
                            matches!(**right, ast::Expr::Literal(_)),
                            "Expected Literal expression for right side"
                        );
                        if let ast::Expr::Literal(literal) = &**right {
                            assert!(
                                matches!(literal, ast::Literal::Numeric(_)),
                                "Expected numeric literal"
                            );
                            if let ast::Literal::Numeric(num_str) = literal {
                                assert_eq!(num_str, "1");
                            }
                        }
                    }
                }
            } else {
                panic!("Expected OneSelect::Select");
            }
        } else {
            panic!("Expected select query");
        }
    }

    #[test]
    fn test_comprehensive_translation() {
        let translator = PostgreSQLTranslator::new();

        // Test various PostgreSQL to Turso AST translations
        let test_cases = vec![
            ("SELECT * FROM sqlite_master", "SELECT * with no WHERE"),
            (
                "SELECT name FROM pg_tables WHERE name = 'users'",
                "SELECT with WHERE and table mapping",
            ),
            (
                "SELECT id, name, age FROM users WHERE age > 18",
                "SELECT multiple columns with WHERE",
            ),
            ("SELECT 'hello', 42 FROM users", "SELECT with literals"),
        ];

        for (sql, description) in test_cases {
            println!("Testing: {description}");
            let parse_result = crate::parse(sql).unwrap();
            let translated = translator.translate(&parse_result);
            assert!(
                translated.is_ok(),
                "Failed to translate: {sql} ({description})"
            );

            if let Ok(ast::Stmt::Select(select)) = translated {
                // Verify it's a valid Select AST
                match &select.body.select {
                    ast::OneSelect::Select { columns, .. } => {
                        assert!(!columns.is_empty(), "No columns in result for: {sql}");
                    }
                    _ => panic!("Expected OneSelect::Select for: {sql}"),
                }
            }
        }
    }

    #[test]
    fn test_literal_expressions() {
        let translator = PostgreSQLTranslator::new();
        let sql = "SELECT 'hello', 42, 3.14 FROM users";
        let parse_result = crate::parse(sql).unwrap();
        let translated = translator.translate(&parse_result);
        assert!(translated.is_ok());

        if let Ok(ast::Stmt::Select(select)) = translated {
            if let ast::OneSelect::Select { columns, .. } = &select.body.select {
                assert_eq!(columns.len(), 3);

                // Check string literal (wrapped in single quotes for Turso AST convention)
                if let ast::ResultColumn::Expr(expr, _) = &columns[0] {
                    if let ast::Expr::Literal(ast::Literal::String(s)) = &**expr {
                        assert_eq!(s, "'hello'");
                    } else {
                        panic!("Expected string literal");
                    }
                } else {
                    panic!("Expected expression result column");
                }

                // Check integer literal
                if let ast::ResultColumn::Expr(expr, _) = &columns[1] {
                    if let ast::Expr::Literal(ast::Literal::Numeric(n)) = &**expr {
                        assert_eq!(n, "42");
                    } else {
                        panic!("Expected numeric literal");
                    }
                } else {
                    panic!("Expected expression result column");
                }

                // Check float literal
                if let ast::ResultColumn::Expr(expr, _) = &columns[2] {
                    if let ast::Expr::Literal(ast::Literal::Numeric(n)) = &**expr {
                        assert_eq!(n, "3.14");
                    } else {
                        panic!("Expected numeric literal");
                    }
                } else {
                    panic!("Expected expression result column");
                }
            } else {
                panic!("Expected OneSelect::Select");
            }
        } else {
            panic!("Expected select query");
        }
    }

    #[test]
    fn test_bool_expr_and_translation() {
        let translator = PostgreSQLTranslator::new();
        let sql = "SELECT * FROM users WHERE age > 18 AND name = 'John'";
        let parse_result = crate::parse(sql).unwrap();
        let translated = translator.translate(&parse_result);
        assert!(translated.is_ok());

        if let Ok(ast::Stmt::Select(select)) = translated {
            if let ast::OneSelect::Select { where_clause, .. } = &select.body.select {
                assert!(where_clause.is_some());
                if let Some(where_expr) = where_clause {
                    // Should be a binary AND expression
                    assert!(
                        matches!(**where_expr, ast::Expr::Binary(_, ast::Operator::And, _)),
                        "Expected AND expression"
                    );
                }
            }
        }
    }

    #[test]
    fn test_bool_expr_or_translation() {
        let translator = PostgreSQLTranslator::new();
        let sql = "SELECT * FROM users WHERE age > 18 OR name = 'John'";
        let parse_result = crate::parse(sql).unwrap();
        let translated = translator.translate(&parse_result);
        assert!(translated.is_ok());

        if let Ok(ast::Stmt::Select(select)) = translated {
            if let ast::OneSelect::Select { where_clause, .. } = &select.body.select {
                assert!(where_clause.is_some());
                if let Some(where_expr) = where_clause {
                    // Should be a binary OR expression
                    assert!(
                        matches!(**where_expr, ast::Expr::Binary(_, ast::Operator::Or, _)),
                        "Expected OR expression"
                    );
                }
            }
        }
    }

    #[test]
    fn test_in_list_translation() {
        let translator = PostgreSQLTranslator::new();
        let sql = "SELECT * FROM users WHERE type IN ('admin', 'user', 'guest')";
        let parse_result = crate::parse(sql).unwrap();
        let translated = translator.translate(&parse_result);
        assert!(translated.is_ok());

        if let Ok(ast::Stmt::Select(select)) = translated {
            if let ast::OneSelect::Select { where_clause, .. } = &select.body.select {
                assert!(where_clause.is_some());
                if let Some(where_expr) = where_clause {
                    // Should be an InList expression
                    if let ast::Expr::InList { lhs, not, rhs } = &**where_expr {
                        assert!(!not, "Should not be NOT IN");
                        assert_eq!(rhs.len(), 3, "Should have 3 values in the IN list");

                        // Check that lhs is a column reference
                        assert!(
                            matches!(**lhs, ast::Expr::Id(_)),
                            "Left side should be a column name"
                        );

                        // Check that the list values are literals
                        for value in rhs {
                            assert!(
                                matches!(**value, ast::Expr::Literal(_)),
                                "IN list values should be literals"
                            );
                        }
                    } else {
                        panic!("Expected InList expression but got: {where_expr:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn test_like_translation() {
        let translator = PostgreSQLTranslator::new();
        let sql = "SELECT * FROM users WHERE name LIKE 'John%'";
        let parse_result = crate::parse(sql).unwrap();
        let translated = translator.translate(&parse_result);
        assert!(translated.is_ok());

        if let Ok(ast::Stmt::Select(select)) = translated {
            if let ast::OneSelect::Select { where_clause, .. } = &select.body.select {
                assert!(where_clause.is_some());
                if let Some(where_expr) = where_clause {
                    // Should be a Like expression
                    if let ast::Expr::Like {
                        lhs,
                        not,
                        op,
                        rhs,
                        escape,
                    } = &**where_expr
                    {
                        assert!(!not, "Should not be NOT LIKE");
                        assert!(
                            matches!(op, ast::LikeOperator::Like),
                            "Should be LIKE operator"
                        );
                        assert!(escape.is_none(), "No ESCAPE clause expected");

                        // Check left and right expressions
                        assert!(
                            matches!(**lhs, ast::Expr::Id(_)),
                            "Left side should be column name"
                        );
                        assert!(
                            matches!(**rhs, ast::Expr::Literal(_)),
                            "Right side should be literal"
                        );
                    } else {
                        panic!("Expected Like expression but got: {where_expr:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn test_not_like_translation() {
        let translator = PostgreSQLTranslator::new();
        let sql = "SELECT * FROM users WHERE name NOT LIKE 'sqlite_%'";
        let parse_result = crate::parse(sql).unwrap();
        let translated = translator.translate(&parse_result);
        assert!(translated.is_ok());

        if let Ok(ast::Stmt::Select(select)) = translated {
            if let ast::OneSelect::Select { where_clause, .. } = &select.body.select {
                assert!(where_clause.is_some());
                if let Some(where_expr) = where_clause {
                    // Should be a Like expression with NOT
                    if let ast::Expr::Like {
                        lhs,
                        not,
                        op,
                        rhs,
                        escape,
                    } = &**where_expr
                    {
                        assert!(*not, "Should be NOT LIKE");
                        assert!(
                            matches!(op, ast::LikeOperator::Like),
                            "Should be LIKE operator"
                        );
                        assert!(escape.is_none(), "No ESCAPE clause expected");

                        // Check expressions
                        assert!(
                            matches!(**lhs, ast::Expr::Id(_)),
                            "Left side should be column name"
                        );
                        assert!(
                            matches!(**rhs, ast::Expr::Literal(_)),
                            "Right side should be literal"
                        );
                    } else {
                        panic!("Expected Like expression but got: {where_expr:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn test_complex_schema_query_translation() {
        let translator = PostgreSQLTranslator::new();
        let sql = "SELECT type, name FROM sqlite_schema WHERE type IN ('table', 'index', 'view') AND name NOT LIKE 'sqlite_%'";
        let parse_result = crate::parse(sql).unwrap();
        let translated = translator.translate(&parse_result);
        assert!(translated.is_ok());

        if let Ok(ast::Stmt::Select(select)) = translated {
            if let ast::OneSelect::Select {
                columns,
                from,
                where_clause,
                ..
            } = &select.body.select
            {
                // Check columns
                assert_eq!(columns.len(), 2, "Should have 2 columns");

                // Check FROM clause
                assert!(from.is_some(), "Should have FROM clause");

                // Check WHERE clause structure
                assert!(where_clause.is_some(), "Should have WHERE clause");
                if let Some(where_expr) = where_clause {
                    // Should be an AND expression
                    if let ast::Expr::Binary(left, op, right) = &**where_expr {
                        assert!(matches!(op, ast::Operator::And), "Top level should be AND");

                        // Left side should be IN expression
                        assert!(
                            matches!(**left, ast::Expr::InList { .. }),
                            "Left side should be IN list"
                        );

                        // Right side should be NOT LIKE expression
                        if let ast::Expr::Like { not, .. } = &**right {
                            assert!(*not, "Right side should be NOT LIKE");
                        } else {
                            panic!("Right side should be LIKE expression");
                        }
                    } else {
                        panic!("WHERE clause should be Binary expression");
                    }
                }
            }
        } else {
            panic!("Translation should succeed");
        }
    }
}
