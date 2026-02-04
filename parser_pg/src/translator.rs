// PostgreSQL AST to Turso AST translator
//
// This module translates pg_query's PostgreSQL AST into Turso's SQL AST
// representation, handling the semantic differences between PostgreSQL and SQLite.

use pg_query::{NodeRef, ParseResult};
use crate::ParseError;
use turso_parser::ast;

/// Translates a PostgreSQL query into Turso's AST
pub struct PostgreSQLTranslator {
    // TODO: Add schema information, type mappings, etc.
}

impl PostgreSQLTranslator {
    pub fn new() -> Self {
        PostgreSQLTranslator {}
    }

    /// Maps PostgreSQL system table names to SQLite equivalents
    fn map_table_name(&self, table_name: &str) -> String {
        match table_name.to_lowercase().as_str() {
            // PostgreSQL information schema mappings
            "pg_tables" => "sqlite_master".to_string(),
            "information_schema.tables" => "sqlite_master".to_string(),
            "pg_class" => "sqlite_master".to_string(),
            "pg_namespace" => "sqlite_master".to_string(),
            "pg_attribute" => "pragma_table_info".to_string(),
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
            },
            NodeRef::InsertStmt(_insert) => Err(ParseError::ParseError("INSERT statements not yet supported".to_string())),
            NodeRef::UpdateStmt(_update) => Err(ParseError::ParseError("UPDATE statements not yet supported".to_string())),
            NodeRef::DeleteStmt(_delete) => Err(ParseError::ParseError("DELETE statements not yet supported".to_string())),
            NodeRef::CreateStmt(_create) => Err(ParseError::ParseError("CREATE statements not yet supported".to_string())),
            _ => Err(ParseError::ParseError(format!(
                "Unsupported statement type: {:?}",
                node.0
            ))),
        }
    }

    fn translate_select(&self, select: &pg_query::protobuf::SelectStmt) -> Result<ast::Select, ParseError> {
        // Translate PostgreSQL SELECT to turso_parser AST

        // 1. Handle FROM clause first to get the base table(s)
        let from_clause = if !select.from_clause.is_empty() {
            // For now, handle single table only
            let from_item = &select.from_clause[0];
            Some(self.translate_from_clause(from_item)?)
        } else {
            None
        };

        // 2. Handle SELECT list (result columns)
        let target_list = &select.target_list;
        if target_list.is_empty() {
            return Err(ParseError::ParseError("Empty target list".to_string()));
        }

        let result_columns = self.translate_target_list(target_list)?;

        // 3. Handle WHERE clause if present
        let where_clause = if let Some(where_clause) = &select.where_clause {
            Some(self.translate_expr(where_clause)?)
        } else {
            None
        };

        // Build the SELECT AST
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

        let select_ast = ast::Select {
            with: None,
            body: select_body,
            order_by: vec![],
            limit: None,
        };

        Ok(select_ast)
    }

    fn translate_from_clause(&self, from_item: &pg_query::protobuf::Node) -> Result<ast::FromClause, ParseError> {
        match &from_item.node {
            Some(pg_query::protobuf::node::Node::RangeVar(range_var)) => {
                let table_name = &range_var.relname;
                let mapped_name = self.map_table_name(table_name);

                let qualified_name = ast::QualifiedName::single(ast::Name::from_string(mapped_name));
                let alias = range_var.alias.as_ref().map(|a| ast::As::Elided(ast::Name::from_string(a.aliasname.clone())));

                let select_table = ast::SelectTable::Table(qualified_name, alias, None);

                Ok(ast::FromClause {
                    select: Box::new(select_table),
                    joins: vec![],
                })
            }
            _ => Err(ParseError::ParseError(format!(
                "Unsupported FROM clause type: {:?}",
                from_item.node
            ))),
        }
    }

    fn translate_target_list(&self, target_list: &[pg_query::protobuf::Node]) -> Result<Vec<ast::ResultColumn>, ParseError> {
        let mut result_columns = Vec::new();

        for target in target_list {
            match &target.node {
                Some(pg_query::protobuf::node::Node::ResTarget(res_target)) => {
                    if let Some(val) = &res_target.val {
                        // Check if this is a SELECT *
                        if let Some(pg_query::protobuf::node::Node::AStar(_)) = &val.node {
                            result_columns.push(ast::ResultColumn::Star);
                        } else if let Some(pg_query::protobuf::node::Node::ColumnRef(col_ref)) = &val.node {
                            // Check if this is a column reference with "*"
                            if let Some(field) = col_ref.fields.first() {
                                if let Some(pg_query::protobuf::node::Node::AStar(_)) = &field.node {
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
                _ => return Err(ParseError::ParseError("Unsupported target list item".to_string())),
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
                                Ok(ast::Expr::Name(ast::Name::from_string(s.sval.clone())))
                            } else {
                                // Qualified column reference (table.column)
                                let mut parts = vec![];
                                for field in &col_ref.fields {
                                    if let Some(pg_query::protobuf::node::Node::String(s)) = &field.node {
                                        parts.push(s.sval.clone());
                                    }
                                }
                                if parts.len() > 1 {
                                    // table.column
                                    Ok(ast::Expr::Qualified(
                                        ast::Name::from_string(parts[0].clone()),
                                        ast::Name::from_string(parts[1].clone())
                                    ))
                                } else {
                                    // Just a simple column name
                                    Ok(ast::Expr::Name(ast::Name::from_string(parts[0].clone())))
                                }
                            }
                        }
                        Some(pg_query::protobuf::node::Node::AStar(_)) => {
                            // SELECT * case - should be handled in translate_target_list, not here
                            return Err(ParseError::ParseError("AStar should be handled in target list, not as expression".to_string()));
                        }
                        _ => Err(ParseError::ParseError(format!("Invalid column reference, expected String or AStar but got: {:?}", field.node))),
                    }
                } else {
                    Err(ParseError::ParseError("Empty column reference".to_string()))
                }
            }
            Some(pg_query::protobuf::node::Node::AConst(a_const)) => {
                self.translate_const(a_const)
            }
            Some(pg_query::protobuf::node::Node::AExpr(a_expr)) => {
                self.translate_binary_expr(a_expr)
            }
            Some(pg_query::protobuf::node::Node::AStar(_)) => {
                // SELECT * - this should be handled as ResultColumn::Star in translate_target_list
                return Err(ParseError::ParseError("AStar should not be translated as expression".to_string()));
            }
            _ => Err(ParseError::ParseError(format!(
                "Unsupported expression type: {:?}",
                node.node
            ))),
        }
    }

    fn translate_const(&self, a_const: &pg_query::protobuf::AConst) -> Result<ast::Expr, ParseError> {
        if let Some(val) = &a_const.val {
            match val {
                pg_query::protobuf::a_const::Val::Ival(i) => {
                    Ok(ast::Expr::Literal(ast::Literal::Numeric(i.ival.to_string())))
                }
                pg_query::protobuf::a_const::Val::Sval(s) => {
                    Ok(ast::Expr::Literal(ast::Literal::String(s.sval.clone())))
                }
                pg_query::protobuf::a_const::Val::Fval(f) => {
                    Ok(ast::Expr::Literal(ast::Literal::Numeric(f.fval.clone())))
                }
                _ => Err(ParseError::ParseError("Unsupported constant type".to_string())),
            }
        } else {
            Err(ParseError::ParseError("Empty constant value".to_string()))
        }
    }

    fn translate_binary_expr(&self, a_expr: &pg_query::protobuf::AExpr) -> Result<ast::Expr, ParseError> {
        // Extract operator name
        let op_name = if let Some(name) = a_expr.name.first() {
            match &name.node {
                Some(pg_query::protobuf::node::Node::String(s)) => &s.sval,
                _ => return Err(ParseError::ParseError("Invalid operator name".to_string())),
            }
        } else {
            return Err(ParseError::ParseError("Missing operator name".to_string()));
        };

        // Map PostgreSQL operators to Turso operators
        let binary_op = match op_name.as_str() {
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
            _ => return Err(ParseError::ParseError(format!("Unsupported operator: {}", op_name))),
        };

        // Translate left and right expressions
        let left = if let Some(lexpr) = &a_expr.lexpr {
            Box::new(self.translate_expr(lexpr)?)
        } else {
            return Err(ParseError::ParseError("Missing left expression".to_string()));
        };

        let right = if let Some(rexpr) = &a_expr.rexpr {
            Box::new(self.translate_expr(rexpr)?)
        } else {
            return Err(ParseError::ParseError("Missing right expression".to_string()));
        };

        Ok(ast::Expr::Binary(left, binary_op, right))
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
    }.to_string()
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
            if let ast::OneSelect::Select { columns, from, where_clause, .. } = &select.body.select {
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
                assert!(matches!(columns[0], ast::ResultColumn::Star), "Expected ResultColumn::Star but got {:?}", columns[0]);

                // Should have FROM clause
                if let Some(from_clause) = from {
                    if let ast::SelectTable::Table(qualified_name, alias, _) = &*from_clause.select {
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
                    assert!(matches!(**expr, ast::Expr::Name(_)), "Expected Name expression but got {:?}", expr);
                    if let ast::Expr::Name(name) = &**expr {
                        assert_eq!(name.as_str(), "id");
                    }
                    assert!(alias.is_none());
                } else {
                    panic!("Expected expression result column for first column");
                }

                // Second column should be 'name'
                if let ast::ResultColumn::Expr(expr, alias) = &columns[1] {
                    assert!(matches!(**expr, ast::Expr::Name(_)), "Expected Name expression but got {:?}", expr);
                    if let ast::Expr::Name(name) = &**expr {
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
                    assert!(matches!(**expr, ast::Expr::Qualified(_, _)), "Expected Qualified expression but got {:?}", expr);
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
                    assert!(matches!(**expr, ast::Expr::Qualified(_, _)), "Expected Qualified expression but got {:?}", expr);
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
            if let ast::OneSelect::Select { columns, from, where_clause, .. } = &select.body.select {
                // Should have SELECT *
                assert_eq!(columns.len(), 1);
                assert!(matches!(columns[0], ast::ResultColumn::Star));

                // Should have FROM clause
                assert!(from.is_some());

                // Should have WHERE clause
                assert!(where_clause.is_some());
                if let Some(where_expr) = where_clause {
                    // WHERE id = 1 should be a binary expression
                    assert!(matches!(**where_expr, ast::Expr::Binary(_, _, _)), "Expected Binary expression but got {:?}", where_expr);
                    if let ast::Expr::Binary(left, op, right) = &**where_expr {
                        // Left side should be column 'id'
                        assert!(matches!(**left, ast::Expr::Name(_)), "Expected Name expression for left side");
                        if let ast::Expr::Name(name) = &**left {
                            assert_eq!(name.as_str(), "id");
                        }

                        // Operator should be Equals
                        assert!(matches!(op, ast::Operator::Equals), "Expected Equals operator");

                        // Right side should be literal 1
                        assert!(matches!(**right, ast::Expr::Literal(_)), "Expected Literal expression for right side");
                        if let ast::Expr::Literal(literal) = &**right {
                            assert!(matches!(literal, ast::Literal::Numeric(_)), "Expected numeric literal");
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
            ("SELECT name FROM pg_tables WHERE name = 'users'", "SELECT with WHERE and table mapping"),
            ("SELECT id, name, age FROM users WHERE age > 18", "SELECT multiple columns with WHERE"),
            ("SELECT 'hello', 42 FROM users", "SELECT with literals"),
        ];

        for (sql, description) in test_cases {
            println!("Testing: {}", description);
            let parse_result = crate::parse(sql).unwrap();
            let translated = translator.translate(&parse_result);
            assert!(translated.is_ok(), "Failed to translate: {} ({})", sql, description);

            if let Ok(ast::Stmt::Select(select)) = translated {
                // Verify it's a valid Select AST
                match &select.body.select {
                    ast::OneSelect::Select { columns, .. } => {
                        assert!(!columns.is_empty(), "No columns in result for: {}", sql);
                    }
                    _ => panic!("Expected OneSelect::Select for: {}", sql),
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

                // Check string literal
                if let ast::ResultColumn::Expr(expr, _) = &columns[0] {
                    if let ast::Expr::Literal(ast::Literal::String(s)) = &**expr {
                        assert_eq!(s, "hello");
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
}