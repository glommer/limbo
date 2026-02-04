// PostgreSQL AST to Turso Logical Plan translator
//
// This module translates pg_query's PostgreSQL AST into Turso's logical plan
// representation, handling the semantic differences between PostgreSQL and SQLite.

use pg_query::{NodeRef, ParseResult};
use crate::ParseError;

/// Translates a PostgreSQL query into Turso's logical plan
pub struct PostgreSQLTranslator {
    // TODO: Add schema information, type mappings, etc.
}

impl PostgreSQLTranslator {
    pub fn new() -> Self {
        PostgreSQLTranslator {}
    }

    /// Translate a PostgreSQL parse result into Turso's format
    pub fn translate(&self, parse_result: &ParseResult) -> Result<TranslatedQuery, ParseError> {
        // The pg_query ParseResult contains a protobuf representation
        // We need to walk the AST nodes and convert them

        if parse_result.protobuf.nodes().is_empty() {
            return Err(ParseError::ParseError("No statements found".to_string()));
        }

        // Get the first statement node
        let node = &parse_result.protobuf.nodes()[0];

        match &node.0 {
            NodeRef::SelectStmt(select) => self.translate_select(select),
            NodeRef::InsertStmt(insert) => self.translate_insert(insert),
            NodeRef::UpdateStmt(update) => self.translate_update(update),
            NodeRef::DeleteStmt(delete) => self.translate_delete(delete),
            NodeRef::CreateStmt(create) => self.translate_create_table(create),
            _ => Err(ParseError::ParseError(format!(
                "Unsupported statement type: {:?}",
                node.0
            ))),
        }
    }

    fn translate_select(&self, _select: &pg_query::protobuf::SelectStmt) -> Result<TranslatedQuery, ParseError> {
        // TODO: Build LogicalPlan for SELECT
        // This involves:
        // 1. Translating FROM clause to TableScan nodes
        // 2. Translating WHERE clause to Filter nodes
        // 3. Translating SELECT list to Projection nodes
        // 4. Handling JOINs, GROUP BY, ORDER BY, etc.

        Ok(TranslatedQuery::Select(SelectQuery {
            // Placeholder - needs full implementation
            sql: "SELECT ...".to_string(),
        }))
    }

    fn translate_insert(&self, _insert: &pg_query::protobuf::InsertStmt) -> Result<TranslatedQuery, ParseError> {
        // TODO: Translate INSERT
        Err(ParseError::ParseError("INSERT translation not yet implemented".to_string()))
    }

    fn translate_update(&self, _update: &pg_query::protobuf::UpdateStmt) -> Result<TranslatedQuery, ParseError> {
        // TODO: Translate UPDATE
        Err(ParseError::ParseError("UPDATE translation not yet implemented".to_string()))
    }

    fn translate_delete(&self, _delete: &pg_query::protobuf::DeleteStmt) -> Result<TranslatedQuery, ParseError> {
        // TODO: Translate DELETE
        Err(ParseError::ParseError("DELETE translation not yet implemented".to_string()))
    }

    fn translate_create_table(&self, _create: &pg_query::protobuf::CreateStmt) -> Result<TranslatedQuery, ParseError> {
        // TODO: Translate CREATE TABLE
        // Need to map PostgreSQL types to SQLite types
        Err(ParseError::ParseError("CREATE TABLE translation not yet implemented".to_string()))
    }
}

/// Result of translating a PostgreSQL query
#[derive(Debug)]
pub enum TranslatedQuery {
    Select(SelectQuery),
    Insert(InsertQuery),
    Update(UpdateQuery),
    Delete(DeleteQuery),
    CreateTable(CreateTableQuery),
}

#[derive(Debug)]
pub struct SelectQuery {
    // TODO: This should contain the LogicalPlan
    pub sql: String,
}

#[derive(Debug)]
pub struct InsertQuery {
    pub sql: String,
}

#[derive(Debug)]
pub struct UpdateQuery {
    pub sql: String,
}

#[derive(Debug)]
pub struct DeleteQuery {
    pub sql: String,
}

#[derive(Debug)]
pub struct CreateTableQuery {
    pub sql: String,
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
    }
}