use crate::common::TempDatabase;
use turso_core::{StepResult, Value};

#[turso_macros::test(mvcc)]
fn test_postgres_pragma(db: TempDatabase) {
    let conn = db.connect_limbo();

    // Test that default dialect is sqlite
    let mut rows = conn.query("PRAGMA sql_dialect").unwrap().unwrap();
    let StepResult::Row = rows.step().unwrap() else {
        panic!("expected row");
    };
    let row = rows.row().unwrap();
    let Value::Text(value) = row.get_value(0) else {
        panic!("expected text value");
    };
    assert_eq!(value.value, "sqlite");
    drop(rows);

    // Switch to postgres dialect
    conn.execute("PRAGMA sql_dialect = postgres").unwrap();

    // Test that PostgreSQL dialect works - try a simple query
    let mut rows = conn.query("SELECT 42").unwrap().unwrap();
    let StepResult::Row = rows.step().unwrap() else {
        panic!("expected row");
    };
    let row = rows.row().unwrap();
    let Value::Integer(value) = row.get_value(0) else {
        panic!("expected integer value");
    };
    assert_eq!(*value, 42);
    drop(rows);

    // Test that PostgreSQL parser rejects PRAGMA statements
    let result = conn.query("PRAGMA table_info(test)");
    assert!(result.is_err(), "PostgreSQL parser should reject PRAGMA statements");
}

#[turso_macros::test(mvcc)]
fn test_postgres_simple_select_literal(db: TempDatabase) {
    let conn = db.connect_limbo();

    // Switch to PostgreSQL dialect
    conn.execute("PRAGMA sql_dialect = postgres").unwrap();

    // Test the simplest possible PostgreSQL query - SELECT literal
    let mut rows = conn.query("SELECT 1").unwrap().unwrap();
    let StepResult::Row = rows.step().unwrap() else {
        panic!("expected row");
    };
    let row = rows.row().unwrap();
    let Value::Integer(value) = row.get_value(0) else {
        panic!("expected integer value");
    };
    assert_eq!(*value, 1);
}


#[turso_macros::test(mvcc)]
fn test_postgres_arithmetic_expression(db: TempDatabase) {
    let conn = db.connect_limbo();

    // Switch to PostgreSQL dialect
    conn.execute("PRAGMA sql_dialect = postgres").unwrap();

    // Test simple arithmetic in PostgreSQL dialect
    let mut rows = conn.query("SELECT 2 + 3").unwrap().unwrap();
    let StepResult::Row = rows.step().unwrap() else {
        panic!("expected row");
    };
    let row = rows.row().unwrap();
    let Value::Integer(result) = row.get_value(0) else {
        panic!("expected integer value");
    };
    assert_eq!(*result, 5);
}

#[turso_macros::test(mvcc)]
fn test_postgres_parser_integration(db: TempDatabase) {
    let conn = db.connect_limbo();

    // Switch to PostgreSQL dialect
    conn.execute("PRAGMA sql_dialect = postgres").unwrap();

    // Test that PostgreSQL parser rejects PRAGMA statements (PostgreSQL doesn't support them)
    let result = conn.query("PRAGMA table_info(test)");
    assert!(result.is_err(), "PostgreSQL parser should reject PRAGMA statements");

    // But should accept PostgreSQL-style comments
    let mut rows = conn.query("SELECT 42 -- PostgreSQL comment").unwrap().unwrap();
    let StepResult::Row = rows.step().unwrap() else {
        panic!("expected row");
    };
    let row = rows.row().unwrap();
    let Value::Integer(value) = row.get_value(0) else {
        panic!("expected integer value");
    };
    assert_eq!(*value, 42);
}
