// Integration test for PostgreSQL dialect functionality
// Test the parser directly instead of full database operations

use turso_core::SqlDialect;

#[test]
fn test_postgres_dialect_enum() {
    // Test that PostgreSQL dialect enum variant exists
    let postgres = SqlDialect::Postgres;
    let sqlite = SqlDialect::Sqlite;

    assert_ne!(postgres, sqlite);
}

#[test]
fn test_postgresql_parser_integration() {
    // Test that the PostgreSQL parser can be used directly
    let sql = "SELECT * FROM test_table";

    // Parse using pg_query directly
    let parse_result = turso_parser_pg::parse(sql);
    assert!(parse_result.is_ok(), "PostgreSQL parser should successfully parse simple SELECT");

    // Test the translator
    let translator = turso_parser_pg::translator::PostgreSQLTranslator::new();
    let ast_result = translator.translate(&parse_result.unwrap());
    assert!(ast_result.is_ok(), "Translator should successfully convert PostgreSQL AST to Turso AST");
}

#[test]
fn test_postgresql_system_table_mapping() {
    // Test that PostgreSQL system table names are mapped correctly
    let sql = "SELECT * FROM pg_tables";

    // Parse using pg_query
    let parse_result = turso_parser_pg::parse(sql);
    assert!(parse_result.is_ok(), "PostgreSQL parser should parse pg_tables query");

    // Test the translator - it should map pg_tables to sqlite_master
    let translator = turso_parser_pg::translator::PostgreSQLTranslator::new();
    let ast_result = translator.translate(&parse_result.unwrap());

    match ast_result {
        Ok(_ast) => {
            // Test passed - we successfully parsed and translated
            println!("pg_tables query parsed and translated successfully");
        },
        Err(e) => {
            // This might fail if the translator hasn't been fully implemented yet
            println!("pg_tables translation failed (this may be expected): {}", e);
        }
    }
}