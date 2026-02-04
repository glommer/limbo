//! Logical Plan to VDBE bytecode compiler
//!
//! This module compiles Turso's LogicalPlan intermediate representation
//! into VDBE bytecode for execution. It provides an alternative path
//! for query execution that goes through the logical plan layer:
//!
//! PostgreSQL/SQLite AST → LogicalPlan → VDBE bytecode
//!
//! Key VDBE instruction patterns used:
//! - Init: Initialize program state
//! - OpenRead: Open table cursor for reading
//! - Rewind: Move cursor to first row
//! - Column: Read column value from current row
//! - Next: Advance cursor to next row
//! - ResultRow: Output a row to result set
//! - Halt: Terminate program

use crate::schema::{BTreeTable, Schema};
use crate::sync::Arc;
use crate::translate::emitter::Resolver;
use crate::translate::expr::translate_expr;
use crate::translate::logical::{LogicalPlan, TableScan, Filter, Projection, LogicalExpr, BinaryOperator};
use crate::vdbe::builder::{CursorType, ProgramBuilder};
use crate::vdbe::insn::Insn;
use crate::vdbe::{BranchOffset, CursorID};
use crate::Result;

/// Context for compiling logical plans to VDBE bytecode
pub struct LogicalCompiler<'a> {
    pub program: &'a mut ProgramBuilder,
    pub resolver: &'a Resolver<'a>,
}

impl<'a> LogicalCompiler<'a> {
    pub fn new(program: &'a mut ProgramBuilder, resolver: &'a Resolver<'a>) -> Self {
        Self { program, resolver }
    }

    /// Compile a logical plan tree into VDBE bytecode
    pub fn compile_plan(&mut self, plan: &LogicalPlan) -> Result<CompilationResult> {
        match plan {
            LogicalPlan::TableScan(table_scan) => self.compile_table_scan(table_scan),
            LogicalPlan::Filter(filter) => self.compile_filter(filter),
            LogicalPlan::Projection(projection) => self.compile_projection(projection),
            _ => todo!("LogicalPlan node not yet supported: {:?}", plan),
        }
    }

    /// Compile a TableScan into VDBE bytecode
    /// Generates: OpenRead → Rewind → [loop: Column reads] → Next → [loop end]
    fn compile_table_scan(&mut self, table_scan: &TableScan) -> Result<CompilationResult> {
        // Look up the table in the schema
        let table = self.resolver.schema.get_table(&table_scan.table_name)
            .ok_or_else(|| crate::LimboError::ParseError(format!("Table not found: {}", table_scan.table_name)))?;

        // Allocate a cursor for the table
        let cursor_id = self.program.alloc_cursor_id(CursorType::BTreeTable(table.clone()));

        // Open the table for reading
        self.program.emit_insn(Insn::OpenRead {
            cursor_id,
            root_page: table.root_page,
            db: 0, // main database
        });

        // Allocate registers for output columns
        let num_columns = table_scan.schema.column_count();
        let result_start_reg = self.program.alloc_registers(num_columns);

        // Set up the scan loop
        let loop_start = self.program.alloc_label();
        let loop_end = self.program.alloc_label();

        // Rewind cursor to the first row
        self.program.emit_insn(Insn::Rewind {
            cursor_id,
            pc_if_empty: BranchOffset::Label(loop_end),
        });

        // Loop start label
        self.program.assign_label(loop_start);

        // Read columns into registers based on projection
        if let Some(projection_indices) = &table_scan.projection {
            // Only read projected columns
            for (output_idx, &column_idx) in projection_indices.iter().enumerate() {
                self.program.emit_insn(Insn::Column {
                    cursor_id,
                    column: column_idx,
                    dest: result_start_reg + output_idx,
                    default: None, // TODO: Handle default values
                });
            }
        } else {
            // Read all columns
            for column_idx in 0..num_columns {
                self.program.emit_insn(Insn::Column {
                    cursor_id,
                    column: column_idx,
                    dest: result_start_reg + column_idx,
                    default: None, // TODO: Handle default values
                });
            }
        }

        // Output the row
        let output_count = table_scan.projection.as_ref().map_or(num_columns, |p| p.len());
        self.program.emit_insn(Insn::ResultRow {
            start_reg: result_start_reg,
            count: output_count,
        });

        // Advance to next row
        self.program.emit_insn(Insn::Next {
            cursor_id,
            pc_if_next: BranchOffset::Label(loop_start),
        });

        // Loop end label
        self.program.assign_label(loop_end);

        Ok(CompilationResult {
            output_start_reg: result_start_reg,
            output_count,
            cursor_id: Some(cursor_id),
        })
    }

    /// Compile a Filter node into VDBE bytecode
    /// Compiles the input plan, then adds conditional jump logic for the predicate
    fn compile_filter(&mut self, filter: &Filter) -> Result<CompilationResult> {
        // For a Filter, we need to modify the compilation strategy.
        // Instead of just compiling the input and then filtering,
        // we need to integrate the filter into the scan loop.

        match &*filter.input {
            LogicalPlan::TableScan(table_scan) => {
                // For table scan + filter, we can generate an optimized loop
                self.compile_table_scan_with_filter(table_scan, &filter.predicate)
            }
            _ => {
                // For other input types, compile input first then add filter
                let input_result = self.compile_plan(&filter.input)?;

                // TODO: This path needs more complex handling for general cases
                // For now, we'll just return the input result
                // In a full implementation, we'd need to:
                // 1. Remove the ResultRow from the input compilation
                // 2. Add predicate evaluation
                // 3. Add conditional ResultRow based on predicate
                Ok(input_result)
            }
        }
    }

    /// Compile a TableScan with an integrated filter predicate
    /// This is more efficient than compiling them separately
    fn compile_table_scan_with_filter(
        &mut self,
        table_scan: &TableScan,
        predicate: &LogicalExpr,
    ) -> Result<CompilationResult> {
        // Look up the table in the schema
        let table = self.resolver.schema.get_table(&table_scan.table_name)
            .ok_or_else(|| crate::LimboError::ParseError(format!("Table not found: {}", table_scan.table_name)))?;

        // Allocate a cursor for the table
        let cursor_id = self.program.alloc_cursor_id(CursorType::BTreeTable(table.clone()));

        // Open the table for reading
        self.program.emit_insn(Insn::OpenRead {
            cursor_id,
            root_page: table.root_page,
            db: 0, // main database
        });

        // Allocate registers for output columns
        let num_columns = table_scan.schema.column_count();
        let result_start_reg = self.program.alloc_registers(num_columns);

        // Allocate a register for the predicate result
        let predicate_reg = self.program.alloc_registers(1);

        // Set up the scan loop
        let loop_start = self.program.alloc_label();
        let loop_end = self.program.alloc_label();
        let check_next = self.program.alloc_label();

        // Rewind cursor to the first row
        self.program.emit_insn(Insn::Rewind {
            cursor_id,
            pc_if_empty: BranchOffset::Label(loop_end),
        });

        // Loop start label
        self.program.assign_label(loop_start);

        // Read columns into registers based on projection
        if let Some(projection_indices) = &table_scan.projection {
            // Only read projected columns
            for (output_idx, &column_idx) in projection_indices.iter().enumerate() {
                self.program.emit_insn(Insn::Column {
                    cursor_id,
                    column: column_idx,
                    dest: result_start_reg + output_idx,
                    default: None, // TODO: Handle default values
                });
            }
        } else {
            // Read all columns
            for column_idx in 0..num_columns {
                self.program.emit_insn(Insn::Column {
                    cursor_id,
                    column: column_idx,
                    dest: result_start_reg + column_idx,
                    default: None, // TODO: Handle default values
                });
            }
        }

        // Evaluate the predicate
        self.compile_logical_expr(predicate, predicate_reg, &result_start_reg, cursor_id)?;

        // If predicate is false (0), jump to check_next
        self.program.emit_insn(Insn::IfNot {
            reg: predicate_reg,
            target_pc: BranchOffset::Label(check_next),
            jump_if_null: false, // Don't jump on NULL (treat as false)
        });

        // Output the row if predicate is true
        let output_count = table_scan.projection.as_ref().map_or(num_columns, |p| p.len());
        self.program.emit_insn(Insn::ResultRow {
            start_reg: result_start_reg,
            count: output_count,
        });

        // Check next label - advance to next row
        self.program.assign_label(check_next);
        self.program.emit_insn(Insn::Next {
            cursor_id,
            pc_if_next: BranchOffset::Label(loop_start),
        });

        // Loop end label
        self.program.assign_label(loop_end);

        Ok(CompilationResult {
            output_start_reg: result_start_reg,
            output_count,
            cursor_id: Some(cursor_id),
        })
    }

    /// Compile a Projection node into VDBE bytecode
    /// Compiles the input plan, then adds expression evaluation for the projected columns
    fn compile_projection(&mut self, projection: &Projection) -> Result<CompilationResult> {
        // For now, we'll implement a simplified version that works with TableScan inputs
        // A full implementation would need to handle arbitrary input plans

        match &*projection.input {
            LogicalPlan::TableScan(table_scan) => {
                // For table scan + projection, we can generate an optimized version
                self.compile_table_scan_with_projection(table_scan, &projection.exprs, &projection.schema)
            }
            LogicalPlan::Filter(filter) => {
                // Handle Filter + Projection by combining them
                match &*filter.input {
                    LogicalPlan::TableScan(table_scan) => {
                        self.compile_table_scan_with_filter_and_projection(
                            table_scan,
                            &filter.predicate,
                            &projection.exprs,
                            &projection.schema,
                        )
                    }
                    _ => {
                        todo!("Complex projection over filtered non-table-scan not yet implemented")
                    }
                }
            }
            _ => {
                // For other input types, we'd need to:
                // 1. Compile the input plan without ResultRow
                // 2. Evaluate projection expressions
                // 3. Emit ResultRow with projected values
                todo!("General projection compilation not yet implemented")
            }
        }
    }

    /// Compile a TableScan with projection expressions
    fn compile_table_scan_with_projection(
        &mut self,
        table_scan: &TableScan,
        projection_exprs: &[LogicalExpr],
        output_schema: &crate::translate::logical::SchemaRef,
    ) -> Result<CompilationResult> {
        // Look up the table in the schema
        let table = self.resolver.schema.get_table(&table_scan.table_name)
            .ok_or_else(|| crate::LimboError::ParseError(format!("Table not found: {}", table_scan.table_name)))?;

        // Allocate a cursor for the table
        let cursor_id = self.program.alloc_cursor_id(CursorType::BTreeTable(table.clone()));

        // Open the table for reading
        self.program.emit_insn(Insn::OpenRead {
            cursor_id,
            root_page: table.root_page,
            db: 0, // main database
        });

        // Allocate registers for reading table columns
        let num_table_columns = table_scan.schema.column_count();
        let table_column_regs = self.program.alloc_registers(num_table_columns);

        // Allocate registers for projection outputs
        let projection_output_regs = self.program.alloc_registers(projection_exprs.len());

        // Set up the scan loop
        let loop_start = self.program.alloc_label();
        let loop_end = self.program.alloc_label();

        // Rewind cursor to the first row
        self.program.emit_insn(Insn::Rewind {
            cursor_id,
            pc_if_empty: BranchOffset::Label(loop_end),
        });

        // Loop start label
        self.program.assign_label(loop_start);

        // Read table columns into registers
        for column_idx in 0..num_table_columns {
            self.program.emit_insn(Insn::Column {
                cursor_id,
                column: column_idx,
                dest: table_column_regs + column_idx,
                default: None, // TODO: Handle default values
            });
        }

        // Evaluate projection expressions
        for (expr_idx, expr) in projection_exprs.iter().enumerate() {
            self.compile_logical_expr(expr, projection_output_regs + expr_idx, &table_column_regs, cursor_id)?;
        }

        // Output the projected row
        self.program.emit_insn(Insn::ResultRow {
            start_reg: projection_output_regs,
            count: projection_exprs.len(),
        });

        // Advance to next row
        self.program.emit_insn(Insn::Next {
            cursor_id,
            pc_if_next: BranchOffset::Label(loop_start),
        });

        // Loop end label
        self.program.assign_label(loop_end);

        Ok(CompilationResult {
            output_start_reg: projection_output_regs,
            output_count: projection_exprs.len(),
            cursor_id: Some(cursor_id),
        })
    }

    /// Compile a TableScan with both filter and projection
    fn compile_table_scan_with_filter_and_projection(
        &mut self,
        table_scan: &TableScan,
        predicate: &LogicalExpr,
        projection_exprs: &[LogicalExpr],
        output_schema: &crate::translate::logical::SchemaRef,
    ) -> Result<CompilationResult> {
        // Look up the table in the schema
        let table = self.resolver.schema.get_table(&table_scan.table_name)
            .ok_or_else(|| crate::LimboError::ParseError(format!("Table not found: {}", table_scan.table_name)))?;

        // Allocate a cursor for the table
        let cursor_id = self.program.alloc_cursor_id(CursorType::BTreeTable(table.clone()));

        // Open the table for reading
        self.program.emit_insn(Insn::OpenRead {
            cursor_id,
            root_page: table.root_page,
            db: 0, // main database
        });

        // Allocate registers for reading table columns
        let num_table_columns = table_scan.schema.column_count();
        let table_column_regs = self.program.alloc_registers(num_table_columns);

        // Allocate registers for projection outputs and predicate
        let projection_output_regs = self.program.alloc_registers(projection_exprs.len());
        let predicate_reg = self.program.alloc_registers(1);

        // Set up the scan loop
        let loop_start = self.program.alloc_label();
        let loop_end = self.program.alloc_label();
        let check_next = self.program.alloc_label();

        // Rewind cursor to the first row
        self.program.emit_insn(Insn::Rewind {
            cursor_id,
            pc_if_empty: BranchOffset::Label(loop_end),
        });

        // Loop start label
        self.program.assign_label(loop_start);

        // Read table columns into registers
        for column_idx in 0..num_table_columns {
            self.program.emit_insn(Insn::Column {
                cursor_id,
                column: column_idx,
                dest: table_column_regs + column_idx,
                default: None, // TODO: Handle default values
            });
        }

        // Evaluate the predicate
        self.compile_logical_expr(predicate, predicate_reg, &table_column_regs, cursor_id)?;

        // If predicate is false (0), jump to check_next
        self.program.emit_insn(Insn::IfNot {
            reg: predicate_reg,
            target_pc: BranchOffset::Label(check_next),
            jump_if_null: false, // Don't jump on NULL (treat as false)
        });

        // Evaluate projection expressions
        for (expr_idx, expr) in projection_exprs.iter().enumerate() {
            self.compile_logical_expr(expr, projection_output_regs + expr_idx, &table_column_regs, cursor_id)?;
        }

        // Output the projected row if predicate is true
        self.program.emit_insn(Insn::ResultRow {
            start_reg: projection_output_regs,
            count: projection_exprs.len(),
        });

        // Check next label - advance to next row
        self.program.assign_label(check_next);
        self.program.emit_insn(Insn::Next {
            cursor_id,
            pc_if_next: BranchOffset::Label(loop_start),
        });

        // Loop end label
        self.program.assign_label(loop_end);

        Ok(CompilationResult {
            output_start_reg: projection_output_regs,
            output_count: projection_exprs.len(),
            cursor_id: Some(cursor_id),
        })
    }

    /// Compile a logical expression into VDBE bytecode that evaluates to a register
    fn compile_logical_expr(
        &mut self,
        expr: &LogicalExpr,
        dest_reg: usize,
        column_regs_start: &usize,
        cursor_id: CursorID,
    ) -> Result<()> {
        match expr {
            LogicalExpr::Literal(value) => {
                // Emit instruction to load literal value into register
                match value {
                    crate::types::Value::Integer(i) => {
                        self.program.emit_insn(Insn::Integer {
                            value: *i,
                            dest: dest_reg,
                        });
                    }
                    crate::types::Value::Real(f) => {
                        self.program.emit_insn(Insn::Real {
                            value: *f,
                            dest: dest_reg,
                        });
                    }
                    crate::types::Value::Text(s) => {
                        self.program.emit_insn(Insn::String8 {
                            value: s.clone(),
                            dest: dest_reg,
                        });
                    }
                    crate::types::Value::Null => {
                        self.program.emit_insn(Insn::Null {
                            dest: dest_reg,
                            dest_end: None,
                        });
                    }
                    crate::types::Value::Blob(_) => {
                        todo!("Blob literals not yet implemented");
                    }
                }
                Ok(())
            }
            LogicalExpr::Column(column_ref) => {
                // For now, we'll need to use the current cursor to read the column
                // This is a simplified implementation - a full version would need proper column resolution

                // For a basic implementation, we'll try to read the column directly from the cursor
                // TODO: This needs proper column resolution with schema lookup

                // For now, assume this is referencing a column from the current table scan
                // We should lookup the column index from the table schema
                let table_name = column_ref.table.as_deref();

                // This is a hack - we need to find the column index somehow
                // In a real implementation, we'd maintain a mapping from columns to register positions
                // For now, we'll emit a Column instruction directly
                self.program.emit_insn(Insn::Column {
                    cursor_id,
                    column: 0, // TODO: Need proper column index resolution
                    dest: dest_reg,
                    default: None,
                });

                // TODO: Implement proper column resolution
                // This requires maintaining schema context and column mappings

                Ok(())
            }
            LogicalExpr::BinaryExpr { left, op, right } => {
                // Allocate temporary registers for operands
                let left_reg = self.program.alloc_registers(1);
                let right_reg = self.program.alloc_registers(1);

                // Compile left and right operands
                self.compile_logical_expr(left, left_reg, column_regs_start, cursor_id)?;
                self.compile_logical_expr(right, right_reg, column_regs_start, cursor_id)?;

                // Emit the appropriate binary operation
                self.compile_binary_op(op, left_reg, right_reg, dest_reg)?;
                Ok(())
            }
            LogicalExpr::UnaryExpr { .. } => {
                todo!("Unary expressions not yet implemented");
            }
            LogicalExpr::AggregateFunction { .. } => {
                todo!("Aggregate functions not yet implemented");
            }
            LogicalExpr::ScalarFunction { .. } => {
                todo!("Scalar functions not yet implemented");
            }
            LogicalExpr::Case { .. } => {
                todo!("CASE expressions not yet implemented");
            }
            LogicalExpr::InList { .. } => {
                todo!("IN expressions not yet implemented");
            }
            LogicalExpr::InSubquery { .. } => {
                todo!("IN subqueries not yet implemented");
            }
            LogicalExpr::Exists { .. } => {
                todo!("EXISTS expressions not yet implemented");
            }
            LogicalExpr::Between { .. } => {
                todo!("BETWEEN expressions not yet implemented");
            }
            LogicalExpr::Cast { .. } => {
                todo!("CAST expressions not yet implemented");
            }
            LogicalExpr::Subquery(_) => {
                todo!("Subquery expressions not yet implemented");
            }
            LogicalExpr::Placeholder(_) => {
                todo!("Placeholder expressions not yet implemented");
            }
        }
    }

    /// Compile a binary operation into VDBE bytecode
    fn compile_binary_op(
        &mut self,
        op: &BinaryOperator,
        left_reg: usize,
        right_reg: usize,
        dest_reg: usize,
    ) -> Result<()> {
        use crate::vdbe::insn::CmpInsFlags;
        use turso_parser::ast;

        match op {
            ast::Operator::Plus => {
                self.program.emit_insn(Insn::Add {
                    lhs: left_reg,
                    rhs: right_reg,
                    dest: dest_reg,
                });
            }
            ast::Operator::Minus => {
                self.program.emit_insn(Insn::Subtract {
                    lhs: left_reg,
                    rhs: right_reg,
                    dest: dest_reg,
                });
            }
            ast::Operator::Star => {
                self.program.emit_insn(Insn::Multiply {
                    lhs: left_reg,
                    rhs: right_reg,
                    dest: dest_reg,
                });
            }
            ast::Operator::Divide => {
                self.program.emit_insn(Insn::Divide {
                    lhs: left_reg,
                    rhs: right_reg,
                    dest: dest_reg,
                });
            }
            ast::Operator::Equals => {
                self.program.emit_insn(Insn::Eq {
                    lhs: left_reg,
                    rhs: right_reg,
                    target_pc: BranchOffset::Offset(2), // Skip next instruction if true
                    flags: CmpInsFlags::default(),
                    collation: None,
                });
                // If equal, set dest_reg to 1
                self.program.emit_insn(Insn::Integer {
                    value: 1,
                    dest: dest_reg,
                });
                // If not equal, set dest_reg to 0 (this instruction is skipped if equal)
                self.program.emit_insn(Insn::Integer {
                    value: 0,
                    dest: dest_reg,
                });
            }
            ast::Operator::NotEquals => {
                self.program.emit_insn(Insn::Ne {
                    lhs: left_reg,
                    rhs: right_reg,
                    target_pc: BranchOffset::Offset(2), // Skip next instruction if not equal
                    flags: CmpInsFlags::default(),
                    collation: None,
                });
                // If not equal, set dest_reg to 1
                self.program.emit_insn(Insn::Integer {
                    value: 1,
                    dest: dest_reg,
                });
                // If equal, set dest_reg to 0 (this instruction is skipped if not equal)
                self.program.emit_insn(Insn::Integer {
                    value: 0,
                    dest: dest_reg,
                });
            }
            ast::Operator::Less => {
                self.program.emit_insn(Insn::Lt {
                    lhs: left_reg,
                    rhs: right_reg,
                    target_pc: BranchOffset::Offset(2),
                    flags: CmpInsFlags::default(),
                    collation: None,
                });
                self.program.emit_insn(Insn::Integer {
                    value: 1,
                    dest: dest_reg,
                });
                self.program.emit_insn(Insn::Integer {
                    value: 0,
                    dest: dest_reg,
                });
            }
            ast::Operator::LessEquals => {
                self.program.emit_insn(Insn::Le {
                    lhs: left_reg,
                    rhs: right_reg,
                    target_pc: BranchOffset::Offset(2),
                    flags: CmpInsFlags::default(),
                    collation: None,
                });
                self.program.emit_insn(Insn::Integer {
                    value: 1,
                    dest: dest_reg,
                });
                self.program.emit_insn(Insn::Integer {
                    value: 0,
                    dest: dest_reg,
                });
            }
            ast::Operator::Greater => {
                self.program.emit_insn(Insn::Gt {
                    lhs: left_reg,
                    rhs: right_reg,
                    target_pc: BranchOffset::Offset(2),
                    flags: CmpInsFlags::default(),
                    collation: None,
                });
                self.program.emit_insn(Insn::Integer {
                    value: 1,
                    dest: dest_reg,
                });
                self.program.emit_insn(Insn::Integer {
                    value: 0,
                    dest: dest_reg,
                });
            }
            ast::Operator::GreaterEquals => {
                self.program.emit_insn(Insn::Ge {
                    lhs: left_reg,
                    rhs: right_reg,
                    target_pc: BranchOffset::Offset(2),
                    flags: CmpInsFlags::default(),
                    collation: None,
                });
                self.program.emit_insn(Insn::Integer {
                    value: 1,
                    dest: dest_reg,
                });
                self.program.emit_insn(Insn::Integer {
                    value: 0,
                    dest: dest_reg,
                });
            }
            ast::Operator::And => {
                // Logical AND: if left is false, result is false; otherwise result is right
                let skip_right_eval = self.program.alloc_label();
                let end_label = self.program.alloc_label();

                // Check if left operand is false (0)
                self.program.emit_insn(Insn::IfNot {
                    reg: left_reg,
                    target_pc: BranchOffset::Label(skip_right_eval),
                    jump_if_null: true, // NULL is treated as false in AND
                });

                // Left is true, so result depends on right operand
                self.program.emit_insn(Insn::Move {
                    from_reg: right_reg,
                    to_reg: dest_reg,
                    count: 1,
                });
                self.program.emit_insn(Insn::Goto {
                    target_pc: BranchOffset::Label(end_label),
                });

                // Left is false, so result is false
                self.program.assign_label(skip_right_eval);
                self.program.emit_insn(Insn::Integer {
                    value: 0,
                    dest: dest_reg,
                });

                self.program.assign_label(end_label);
            }
            ast::Operator::Or => {
                // Logical OR: if left is true, result is true; otherwise result is right
                let skip_right_eval = self.program.alloc_label();
                let end_label = self.program.alloc_label();

                // Check if left operand is true (non-zero)
                self.program.emit_insn(Insn::If {
                    reg: left_reg,
                    target_pc: BranchOffset::Label(skip_right_eval),
                    jump_if_null: false, // Don't treat NULL as true
                });

                // Left is false/null, so result depends on right operand
                self.program.emit_insn(Insn::Move {
                    from_reg: right_reg,
                    to_reg: dest_reg,
                    count: 1,
                });
                self.program.emit_insn(Insn::Goto {
                    target_pc: BranchOffset::Label(end_label),
                });

                // Left is true, so result is true
                self.program.assign_label(skip_right_eval);
                self.program.emit_insn(Insn::Integer {
                    value: 1,
                    dest: dest_reg,
                });

                self.program.assign_label(end_label);
            }
            _ => {
                todo!("Binary operator not yet implemented: {:?}", op);
            }
        }
        Ok(())
    }
}

/// Result of compiling a logical plan node
#[derive(Debug)]
pub struct CompilationResult {
    /// Starting register for output columns
    pub output_start_reg: usize,
    /// Number of output columns
    pub output_count: usize,
    /// Cursor ID if this node opened a cursor
    pub cursor_id: Option<CursorID>,
}

/// Compile a complete logical plan into a VDBE program
pub fn compile_logical_plan(
    plan: &LogicalPlan,
    program: &mut ProgramBuilder,
    resolver: &Resolver,
) -> Result<()> {
    let mut compiler = LogicalCompiler::new(program, resolver);

    // Compile the logical plan tree
    let _result = compiler.compile_plan(plan)?;

    // Add program termination
    program.emit_insn(Insn::Halt {
        err_code: 0,
        description: "Logical plan execution completed".to_string(),
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Column, Schema, Type};
    use crate::translate::logical::{ColumnInfo, LogicalSchema};
    use crate::vdbe::builder::{ProgramBuilder, ProgramBuilderOpts, QueryMode};
    use std::sync::Arc;

    fn create_test_schema() -> Arc<Schema> {
        let mut schema = Schema::new();

        // Create a test table with some columns
        let columns = vec![
            Column::new("id".to_string(), Type::Integer, true, None, false),
            Column::new("name".to_string(), Type::Text, false, None, false),
            Column::new("age".to_string(), Type::Integer, false, None, false),
        ];

        let table = Table::new(
            "test_table".to_string(),
            columns,
            vec![], // indexes
            false,  // without_rowid
            false,  // strict
        );

        schema.add_table("test_table".to_string(), table);
        Arc::new(schema)
    }

    fn create_test_program_builder() -> ProgramBuilder {
        ProgramBuilder::new(
            QueryMode::Execute,
            Arc::new(crate::storage::wal::DataChangeMonitor::new()),
            ProgramBuilderOpts {
                num_cursors: 5,
                approx_num_insns: 50,
                approx_num_labels: 10,
            },
        )
    }

    #[test]
    fn test_table_scan_compilation() {
        let schema = create_test_schema();
        let mut program = create_test_program_builder();
        let resolver = Resolver::new(&schema, &crate::SymbolTable::new());

        // Create a logical schema that matches our test table
        let logical_columns = vec![
            ColumnInfo {
                name: "id".to_string(),
                ty: Type::Integer,
                database: None,
                table: Some("test_table".to_string()),
                table_alias: None,
            },
            ColumnInfo {
                name: "name".to_string(),
                ty: Type::Text,
                database: None,
                table: Some("test_table".to_string()),
                table_alias: None,
            },
            ColumnInfo {
                name: "age".to_string(),
                ty: Type::Integer,
                database: None,
                table: Some("test_table".to_string()),
                table_alias: None,
            },
        ];
        let logical_schema = Arc::new(LogicalSchema::new(logical_columns));

        // Create a TableScan logical plan
        let table_scan = TableScan {
            table_name: "test_table".to_string(),
            alias: None,
            schema: logical_schema,
            projection: None, // Read all columns
        };

        // Compile the table scan
        let mut compiler = LogicalCompiler::new(&mut program, &resolver);
        let result = compiler.compile_table_scan(&table_scan);

        // Verify compilation succeeded
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.output_count, 3); // Should output all 3 columns
        assert!(result.cursor_id.is_some()); // Should have allocated a cursor
    }

    #[test]
    fn test_table_scan_with_projection() {
        let schema = create_test_schema();
        let mut program = create_test_program_builder();
        let resolver = Resolver::new(&schema, &crate::SymbolTable::new());

        // Create logical schema with only projected columns
        let logical_columns = vec![
            ColumnInfo {
                name: "id".to_string(),
                ty: Type::Integer,
                database: None,
                table: Some("test_table".to_string()),
                table_alias: None,
            },
            ColumnInfo {
                name: "name".to_string(),
                ty: Type::Text,
                database: None,
                table: Some("test_table".to_string()),
                table_alias: None,
            },
        ];
        let logical_schema = Arc::new(LogicalSchema::new(logical_columns));

        // Create a TableScan with projection (only columns 0 and 1)
        let table_scan = TableScan {
            table_name: "test_table".to_string(),
            alias: None,
            schema: logical_schema,
            projection: Some(vec![0, 1]), // Only read id and name columns
        };

        // Compile the table scan
        let mut compiler = LogicalCompiler::new(&mut program, &resolver);
        let result = compiler.compile_table_scan(&table_scan);

        // Verify compilation succeeded
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.output_count, 2); // Should output only 2 projected columns
        assert!(result.cursor_id.is_some()); // Should have allocated a cursor
    }
}