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

    // Verify it switched
    let mut rows = conn.query("PRAGMA sql_dialect").unwrap().unwrap();
    let StepResult::Row = rows.step().unwrap() else {
        panic!("expected row");
    };
    let row = rows.row().unwrap();
    let Value::Text(value) = row.get_value(0) else {
        panic!("expected text value");
    };
    assert_eq!(value.value, "postgres");
    drop(rows);

    // Switch back to sqlite
    conn.execute("PRAGMA sql_dialect = sqlite").unwrap();

    // Verify it switched back
    let mut rows = conn.query("PRAGMA sql_dialect").unwrap().unwrap();
    let StepResult::Row = rows.step().unwrap() else {
        panic!("expected row");
    };
    let row = rows.row().unwrap();
    let Value::Text(value) = row.get_value(0) else {
        panic!("expected text value");
    };
    assert_eq!(value.value, "sqlite");
}

/* These tests will be enabled once the wiring is complete
#[test]
#[ignore] // This test won't work until we wire everything together
fn test_postgres_select_sqlite_master() -> Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    // Create a new database connection
    let conn = Rc::new(Connection::open(db_path)?);

    // Create a test table in SQLite dialect
    conn.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")?;

    // Switch to PostgreSQL dialect
    conn.execute("PRAGMA sql_dialect = postgres")?;

    // Query using PostgreSQL syntax (pg_tables should map to sqlite_master)
    let rows = conn.execute("SELECT name FROM pg_tables WHERE type = 'table'")?;

    // We should find the users table
    assert!(rows.iter().any(|row| row[0].to_string() == "users"));

    Ok(())
}

#[test]
#[ignore] // This test won't work until we wire everything together
fn test_postgres_simple_select() -> Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");

    // Create a new database connection
    let conn = Rc::new(Connection::open(db_path)?);

    // Create and populate a test table
    conn.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")?;
    conn.execute("INSERT INTO users (id, name) VALUES (1, 'Alice')")?;
    conn.execute("INSERT INTO users (id, name) VALUES (2, 'Bob')")?;

    // Switch to PostgreSQL dialect
    conn.execute("PRAGMA sql_dialect = postgres")?;

    // Query using PostgreSQL syntax
    let rows = conn.execute("SELECT * FROM users WHERE id = 1")?;

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0].to_string(), "1");
    assert_eq!(rows[0][1].to_string(), "Alice");

    Ok(())
}
*/
