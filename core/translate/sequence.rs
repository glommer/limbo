use std::sync::Arc;

use crate::bail_parse_error;
use crate::schema::{BTreeTable, Sequence};
use crate::storage::pager::CreateBTreeFlags;
use crate::translate::emitter::Resolver;
use crate::translate::schema::{emit_schema_entry, SchemaEntryType, SQLITE_TABLEID};
use crate::util::{escape_sql_string_literal, normalize_ident};
use crate::vdbe::builder::{CursorType, ProgramBuilder};
use crate::vdbe::insn::{to_u16, CmpInsFlags, Cookie, InsertFlags, Insn, RegisterOrLiteral};
use crate::Result;
use turso_parser::ast;

/// Build the CREATE TABLE SQL that backs a sequence.
///
/// The table has a fixed set of `__turso_seq_*` columns that encode the
/// sequence configuration. SQLite sees a normal table; Turso detects the
/// sentinel first column during schema loading and treats it as a sequence.
fn sequence_backing_table_sql(name: &str) -> String {
    format!(
        "CREATE TABLE \"{name}\"(\
         __turso_seq_value INTEGER,\
         __turso_seq_is_called INTEGER,\
         __turso_seq_start INTEGER,\
         __turso_seq_inc INTEGER,\
         __turso_seq_min INTEGER,\
         __turso_seq_max INTEGER,\
         __turso_seq_cache INTEGER,\
         __turso_seq_cycle INTEGER)"
    )
}

/// Translate CREATE SEQUENCE into bytecode that persists the definition in sqlite_schema.
#[allow(clippy::too_many_arguments)]
pub fn translate_create_sequence(
    seq_name: &ast::QualifiedName,
    if_not_exists: bool,
    start: &Option<i64>,
    increment: &Option<i64>,
    min_value: &Option<i64>,
    max_value: &Option<i64>,
    cache: &Option<i64>,
    cycle: bool,
    resolver: &Resolver,
    program: &mut ProgramBuilder,
) -> Result<()> {
    let database_id = resolver.resolve_database_id(seq_name)?;
    let schema_cookie = resolver.with_schema(database_id, |s| s.schema_version);
    program.begin_write_on_database(database_id, schema_cookie);

    let normalized_name = normalize_ident(seq_name.name.as_str());

    // Check if sequence already exists
    let exists = resolver.with_schema(database_id, |s| s.get_sequence(&normalized_name).is_some());
    if exists {
        if if_not_exists {
            return Ok(());
        }
        bail_parse_error!("sequence \"{}\" already exists", normalized_name);
    }

    // Validate parameters early (gives immediate error instead of deferred)
    let seq = Sequence::new(
        normalized_name.clone(),
        *start,
        *increment,
        *min_value,
        *max_value,
        *cache,
        cycle,
    )?;

    // Build the CREATE TABLE SQL for persistence
    let sql = sequence_backing_table_sql(&normalized_name);

    // Allocate a real B-tree root page (just like regular tables)
    let table_root_reg = program.alloc_register();
    program.emit_insn(Insn::CreateBtree {
        db: database_id,
        root: table_root_reg,
        flags: CreateBTreeFlags::new_table(),
    });

    // Open cursor to sqlite_schema (in the target database)
    let table = resolver.with_schema(database_id, |s| s.get_btree_table(SQLITE_TABLEID).unwrap());
    let sqlite_schema_cursor_id = program.alloc_cursor_id(CursorType::BTreeTable(table));
    program.emit_insn(Insn::OpenWrite {
        cursor_id: sqlite_schema_cursor_id,
        root_page: 1i64.into(),
        db: database_id,
    });

    // Write entry: type='table', name=seq_name, tbl_name=seq_name, rootpage=<real>, sql=CREATE TABLE ...
    emit_schema_entry(
        program,
        resolver,
        sqlite_schema_cursor_id,
        None,
        SchemaEntryType::Table,
        &normalized_name,
        &normalized_name,
        table_root_reg,
        Some(sql),
    )?;

    // Insert the initial row with sequence config values.
    // We open the newly-created table by its root page register and insert one row.
    let seq_btree = Arc::new(BTreeTable::from_sql(
        &sequence_backing_table_sql(&normalized_name),
        0,
    )?);
    let seq_cursor_id = program.alloc_cursor_id(CursorType::BTreeTable(seq_btree));
    program.emit_insn(Insn::OpenWrite {
        cursor_id: seq_cursor_id,
        root_page: RegisterOrLiteral::Register(table_root_reg),
        db: database_id,
    });

    // Build the record: (value, is_called, start, inc, min, max, cache, cycle)
    let base_reg = program.alloc_registers(8);
    program.emit_insn(Insn::Integer {
        dest: base_reg,
        value: seq.start_value,
    });
    program.emit_insn(Insn::Integer {
        dest: base_reg + 1,
        value: 0, // is_called = false
    });
    program.emit_insn(Insn::Integer {
        dest: base_reg + 2,
        value: seq.start_value,
    });
    program.emit_insn(Insn::Integer {
        dest: base_reg + 3,
        value: seq.increment_by,
    });
    program.emit_insn(Insn::Integer {
        dest: base_reg + 4,
        value: seq.min_value,
    });
    program.emit_insn(Insn::Integer {
        dest: base_reg + 5,
        value: seq.max_value,
    });
    program.emit_insn(Insn::Integer {
        dest: base_reg + 6,
        value: seq.cache,
    });
    program.emit_insn(Insn::Integer {
        dest: base_reg + 7,
        value: if seq.cycle { 1 } else { 0 },
    });

    let record_reg = program.alloc_register();
    program.emit_insn(Insn::MakeRecord {
        start_reg: to_u16(base_reg),
        count: 8,
        dest_reg: to_u16(record_reg),
        index_name: None,
        affinity_str: None,
    });

    let rowid_reg = program.alloc_register();
    program.emit_insn(Insn::NewRowid {
        cursor: seq_cursor_id,
        rowid_reg,
        prev_largest_reg: 0,
    });

    program.emit_insn(Insn::Insert {
        cursor: seq_cursor_id,
        key_reg: rowid_reg,
        record_reg,
        flag: InsertFlags::new(),
        table_name: normalized_name.clone(),
    });

    // ParseSchema to add the backing table to Schema.tables
    let escaped = escape_sql_string_literal(&normalized_name);
    program.emit_insn(Insn::ParseSchema {
        db: database_id,
        where_clause: Some(format!("name = '{escaped}'")),
    });

    // Add the fully-configured Sequence to the in-memory schema
    program.emit_insn(Insn::AddSequence {
        db: database_id,
        name: normalized_name.clone(),
        start: seq.start_value,
        increment: seq.increment_by,
        min_value: seq.min_value,
        max_value: seq.max_value,
        cache: seq.cache,
        cycle: seq.cycle,
    });

    // Bump schema version so other connections detect the change
    program.emit_insn(Insn::SetCookie {
        db: database_id,
        cookie: Cookie::SchemaVersion,
        value: schema_cookie as i32 + 1,
        p5: 0,
    });

    Ok(())
}

/// Translate DROP SEQUENCE into bytecode that removes the definition from sqlite_schema.
pub fn translate_drop_sequence(
    seq_name: &ast::QualifiedName,
    if_exists: bool,
    resolver: &Resolver,
    program: &mut ProgramBuilder,
) -> Result<()> {
    let database_id = resolver.resolve_database_id(seq_name)?;
    let schema_cookie = resolver.with_schema(database_id, |s| s.schema_version);
    program.begin_write_on_database(database_id, schema_cookie);

    let normalized_name = normalize_ident(seq_name.name.as_str());

    // Check existence and get the root page from the backing table
    let root_page = resolver.with_schema(database_id, |s| {
        s.get_sequence(&normalized_name)?;
        Some(s.get_btree_table(&normalized_name)?.root_page)
    });

    if root_page.is_none() {
        if if_exists {
            return Ok(());
        }
        bail_parse_error!("sequence \"{}\" does not exist", normalized_name);
    }
    let root_page = root_page.unwrap();

    // Open sqlite_schema for writing (in the target database)
    let schema_table =
        resolver.with_schema(database_id, |s| s.get_btree_table(SQLITE_TABLEID).unwrap());
    let sqlite_schema_cursor_id = program.alloc_cursor_id(CursorType::BTreeTable(schema_table));
    program.emit_insn(Insn::OpenWrite {
        cursor_id: sqlite_schema_cursor_id,
        root_page: 1i64.into(),
        db: database_id,
    });

    // Allocate registers for searching
    let seq_name_reg = program.alloc_register();
    let type_str_reg = program.alloc_register();
    let rowid_reg = program.alloc_register();

    program.emit_insn(Insn::String8 {
        dest: seq_name_reg,
        value: normalized_name.clone(),
    });
    program.emit_insn(Insn::String8 {
        dest: type_str_reg,
        value: "table".to_string(),
    });

    // Scan sqlite_schema for type='table' AND name=normalized_name, then delete
    let end_loop_label = program.allocate_label();
    let loop_start_label = program.allocate_label();

    program.emit_insn(Insn::Rewind {
        cursor_id: sqlite_schema_cursor_id,
        pc_if_empty: end_loop_label,
    });
    program.preassign_label_to_next_insn(loop_start_label);

    // Column 0 = type, Column 1 = name
    let col0_reg = program.alloc_register();
    let col1_reg = program.alloc_register();

    program.emit_column_or_rowid(sqlite_schema_cursor_id, 0, col0_reg);
    program.emit_column_or_rowid(sqlite_schema_cursor_id, 1, col1_reg);

    let skip_delete_label = program.allocate_label();

    // Compare type
    program.emit_insn(Insn::Ne {
        lhs: col0_reg,
        rhs: type_str_reg,
        target_pc: skip_delete_label,
        flags: CmpInsFlags::default(),
        collation: program.curr_collation(),
    });

    // Compare name
    program.emit_insn(Insn::Ne {
        lhs: col1_reg,
        rhs: seq_name_reg,
        target_pc: skip_delete_label,
        flags: CmpInsFlags::default(),
        collation: program.curr_collation(),
    });

    // Found — delete the row
    program.emit_insn(Insn::RowId {
        cursor_id: sqlite_schema_cursor_id,
        dest: rowid_reg,
    });
    program.emit_insn(Insn::Delete {
        cursor_id: sqlite_schema_cursor_id,
        table_name: "sqlite_schema".to_string(),
        is_part_of_update: false,
    });

    program.preassign_label_to_next_insn(skip_delete_label);
    program.emit_insn(Insn::Next {
        cursor_id: sqlite_schema_cursor_id,
        pc_if_next: loop_start_label,
    });

    program.preassign_label_to_next_insn(end_loop_label);

    // Destroy the B-tree root page
    let former_root_reg = program.alloc_register();
    program.emit_insn(Insn::Destroy {
        db: database_id,
        root: root_page,
        former_root_reg,
        is_temp: 0,
    });

    // Remove from the in-memory schema (sequences + tables maps)
    program.emit_insn(Insn::DropSequence {
        db: database_id,
        seq_name: normalized_name,
    });

    // Bump schema version so other connections detect the change
    program.emit_insn(Insn::SetCookie {
        db: database_id,
        cookie: Cookie::SchemaVersion,
        value: schema_cookie as i32 + 1,
        p5: 0,
    });

    Ok(())
}
