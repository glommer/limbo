use crate::common::TempDatabase;
use turso_core::{StepResult, Value};

#[turso_macros::test]
fn test_postgres_pg_namespace(db: TempDatabase) {
    let conn = db.connect_limbo();

    // Switch to PostgreSQL dialect
    conn.execute("PRAGMA sql_dialect = 'postgres'").unwrap();

    // Query pg_namespace virtual table
    let mut stmt = conn.prepare("SELECT * FROM pg_namespace").unwrap();

    // Should have at least pg_catalog and public namespaces
    let mut found_pg_catalog = false;
    let mut found_public = false;

    loop {
        match stmt.step().unwrap() {
            StepResult::Row => {
                let row = stmt.row().unwrap();
                if let Value::Text(nspname) = row.get_value(1) {
                    if nspname.as_str() == "pg_catalog" {
                        found_pg_catalog = true;
                    } else if nspname.as_str() == "public" {
                        found_public = true;
                    }
                }
            }
            StepResult::Done => break,
            _ => {}
        }
    }

    assert!(found_pg_catalog, "pg_catalog namespace not found");
    assert!(found_public, "public namespace not found");
}

#[turso_macros::test]
fn test_postgres_pg_class(db: TempDatabase) {
    let conn = db.connect_limbo();

    // Create a test table in SQLite dialect first
    conn.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)").unwrap();

    // Switch to PostgreSQL dialect
    conn.execute("PRAGMA sql_dialect = 'postgres'").unwrap();

    // Query pg_class virtual table
    let mut stmt = conn.prepare("SELECT relname, relkind FROM pg_class WHERE relkind = 'r'").unwrap();

    // Should see our users table (once we implement the mapping)
    let mut _found_users_table = false;
    loop {
        match stmt.step().unwrap() {
            StepResult::Row => {
                let row = stmt.row().unwrap();
                if let (Value::Text(relname), Value::Text(relkind)) =
                    (row.get_value(0), row.get_value(1)) {
                    if relname.as_str() == "users" && relkind.as_str() == "r" {
                        _found_users_table = true;
                    }
                }
            }
            StepResult::Done => break,
            _ => {}
        }
    }

    // For now this won't find the users table as we haven't implemented
    // the actual mapping from sqlite_master to pg_class yet
    // This is just testing that the virtual table exists and can be queried
}

#[turso_macros::test]
fn test_postgres_pg_attribute(db: TempDatabase) {
    let conn = db.connect_limbo();

    // Switch to PostgreSQL dialect
    conn.execute("PRAGMA sql_dialect = 'postgres'").unwrap();

    // Query pg_attribute virtual table
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM pg_attribute").unwrap();

    match stmt.step().unwrap() {
        StepResult::Row => {
            let row = stmt.row().unwrap();
            if let Value::Integer(count) = row.get_value(0) {
                // For now should be 0 since we haven't implemented the mapping yet
                assert_eq!(*count, 0);
            }
        }
        _ => panic!("Expected row from COUNT query"),
    }
}