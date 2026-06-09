//! Tests for Turso-specific statement result-column metadata APIs and
//! function-name aliases.
//!
//! Two pieces of public surface bundled because they share the same testing
//! pattern: prepare a statement, inspect column metadata or invoke aliased
//! functions, assert the result.
//!
//! - `Statement::get_column_type_info`: returns rich type info (declared
//!   name, array depth, resolved custom-type base, type kind) for direct
//!   table-column references. Complements the SQLite-compat
//!   `get_column_decltype`, which intentionally returns only the bare
//!   declared name to preserve ABI.
//! - Function-name aliases: `chr` resolves to the same scalar function as
//!   `char`, and `strpos` resolves to the same scalar function as `instr`.

#[cfg(test)]
mod tests {
    use crate::common::{ExecRows, TempDatabase};
    use tempfile::TempDir;
    use turso_core::ColumnTypeKind;

    /// Helper: open a fresh DB with custom types + STRICT support enabled.
    /// Array, struct, union and domain columns require STRICT mode and the
    /// `custom_types` opt.
    fn fresh_db_with_custom_types(name: &str) -> TempDatabase {
        let path = TempDir::new().unwrap().keep().join(name);
        let opts = turso_core::DatabaseOpts::new().with_custom_types(true);
        TempDatabase::new_with_existent_with_opts(&path, opts)
    }

    /// Built-in scalar columns: `array_dimensions == 0`, no `base_type`
    /// (declared name is itself the primitive), `kind == Builtin`.
    #[test]
    fn type_info_for_builtin_scalar_columns() {
        let db = fresh_db_with_custom_types("type_info_builtin_scalar.db");
        let conn = db.connect_limbo();
        conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, label TEXT) STRICT")
            .unwrap();

        let stmt = conn.prepare("SELECT id, label FROM t").unwrap();

        let id_info = stmt.get_column_type_info(0).unwrap();
        assert_eq!(id_info.declared_name, "INTEGER");
        assert_eq!(id_info.array_dimensions, 0);
        assert_eq!(id_info.base_type, None);
        assert_eq!(id_info.kind, ColumnTypeKind::Builtin);

        let label_info = stmt.get_column_type_info(1).unwrap();
        assert_eq!(label_info.declared_name, "TEXT");
        assert_eq!(label_info.array_dimensions, 0);
        assert_eq!(label_info.base_type, None);
        assert_eq!(label_info.kind, ColumnTypeKind::Builtin);
    }

    /// Array columns report `array_dimensions` matching the bracket depth in
    /// CREATE TABLE. The declared name is still the element type (no
    /// brackets), preserving compatibility with how `get_column_decltype`
    /// already names the type.
    #[test]
    fn type_info_for_array_columns() {
        let db = fresh_db_with_custom_types("type_info_arrays.db");
        let conn = db.connect_limbo();
        conn.execute(
            "CREATE TABLE t(\
                 arr_1d INTEGER[],\
                 arr_2d TEXT[][]\
             ) STRICT",
        )
        .unwrap();

        let stmt = conn.prepare("SELECT arr_1d, arr_2d FROM t").unwrap();

        let one_d = stmt.get_column_type_info(0).unwrap();
        assert_eq!(one_d.declared_name, "INTEGER");
        assert_eq!(one_d.array_dimensions, 1);
        assert_eq!(one_d.base_type, None);
        assert_eq!(one_d.kind, ColumnTypeKind::Builtin);

        let two_d = stmt.get_column_type_info(1).unwrap();
        assert_eq!(two_d.declared_name, "TEXT");
        assert_eq!(two_d.array_dimensions, 2);
        assert_eq!(two_d.base_type, None);
        assert_eq!(two_d.kind, ColumnTypeKind::Builtin);
    }

    /// A column whose declared type is a `CREATE TYPE ... BASE ...` reports
    /// the declared name verbatim, resolves `base_type` to the underlying
    /// primitive, and reports `kind == Custom`.
    #[test]
    fn type_info_for_custom_type_columns() {
        let db = fresh_db_with_custom_types("type_info_custom.db");
        let conn = db.connect_limbo();
        conn.execute("CREATE TYPE cents BASE integer ENCODE value * 100 DECODE value / 100")
            .unwrap();
        conn.execute("CREATE TABLE accounts(id INTEGER PRIMARY KEY, amount cents) STRICT")
            .unwrap();

        let stmt = conn.prepare("SELECT id, amount FROM accounts").unwrap();

        let id_info = stmt.get_column_type_info(0).unwrap();
        assert_eq!(id_info.declared_name, "INTEGER");
        assert_eq!(id_info.base_type, None);
        assert_eq!(id_info.kind, ColumnTypeKind::Builtin);

        let amount_info = stmt.get_column_type_info(1).unwrap();
        assert_eq!(amount_info.declared_name, "cents");
        assert_eq!(amount_info.array_dimensions, 0);
        assert_eq!(amount_info.base_type, Some("INTEGER".to_string()));
        assert_eq!(amount_info.kind, ColumnTypeKind::Custom);
    }

    /// `CREATE DOMAIN` columns share the same underlying primitive as their
    /// base type but report `kind == Domain` — letting consumers know there
    /// are CHECK constraints attached even though the on-disk representation
    /// is identical to the base.
    #[test]
    fn type_info_for_domain_columns() {
        let db = fresh_db_with_custom_types("type_info_domain.db");
        let conn = db.connect_limbo();
        conn.execute("CREATE DOMAIN positive_int AS integer CHECK (VALUE > 0)")
            .unwrap();
        conn.execute("CREATE TABLE t(id INTEGER PRIMARY KEY, score positive_int) STRICT")
            .unwrap();

        let stmt = conn.prepare("SELECT score FROM t").unwrap();
        let score_info = stmt.get_column_type_info(0).unwrap();
        assert_eq!(score_info.declared_name, "positive_int");
        assert_eq!(score_info.base_type, Some("INTEGER".to_string()));
        assert_eq!(score_info.kind, ColumnTypeKind::Domain);
    }

    /// `CREATE TYPE ... AS STRUCT(...)` columns are stored as BLOBs but
    /// report `kind == Struct` so wire-protocol layers can map them to
    /// composite / JSON types instead of raw BYTEA.
    #[test]
    fn type_info_for_struct_columns() {
        let db = fresh_db_with_custom_types("type_info_struct.db");
        let conn = db.connect_limbo();
        conn.execute("CREATE TYPE point AS STRUCT(x INT, y INT)")
            .unwrap();
        conn.execute("CREATE TABLE t(id INT, pos point) STRICT")
            .unwrap();

        let stmt = conn.prepare("SELECT pos FROM t").unwrap();
        let pos_info = stmt.get_column_type_info(0).unwrap();
        assert_eq!(pos_info.declared_name, "point");
        assert_eq!(pos_info.array_dimensions, 0);
        // Structs flatten to BLOB on disk, but `kind` exposes the original
        // shape so callers can tell apart a struct column from a plain BLOB.
        assert_eq!(pos_info.base_type, Some("BLOB".to_string()));
        assert_eq!(pos_info.kind, ColumnTypeKind::Struct);
    }

    /// `CREATE TYPE ... AS UNION(...)` columns are stored as BLOBs but
    /// report `kind == Union`, distinguishing them from struct columns and
    /// plain BLOBs.
    #[test]
    fn type_info_for_union_columns() {
        let db = fresh_db_with_custom_types("type_info_union.db");
        let conn = db.connect_limbo();
        conn.execute("CREATE TYPE shape AS UNION(circle INT, square INT)")
            .unwrap();
        conn.execute("CREATE TABLE t(id INT, s shape) STRICT")
            .unwrap();

        let stmt = conn.prepare("SELECT s FROM t").unwrap();
        let s_info = stmt.get_column_type_info(0).unwrap();
        assert_eq!(s_info.declared_name, "shape");
        assert_eq!(s_info.array_dimensions, 0);
        assert_eq!(s_info.base_type, Some("BLOB".to_string()));
        assert_eq!(s_info.kind, ColumnTypeKind::Union);
    }

    /// Computed expressions, subqueries, and other non-table-column result
    /// columns return `None` — there is no schema column to inspect.
    #[test]
    fn type_info_none_for_expressions() {
        let db = fresh_db_with_custom_types("type_info_expressions.db");
        let conn = db.connect_limbo();
        conn.execute("CREATE TABLE t(id INTEGER) STRICT").unwrap();

        let stmt = conn.prepare("SELECT id + 1, 42, 'literal' FROM t").unwrap();

        assert_eq!(
            stmt.get_column_type_info(0),
            None,
            "id + 1 is an expression"
        );
        assert_eq!(stmt.get_column_type_info(1), None, "integer literal");
        assert_eq!(stmt.get_column_type_info(2), None, "string literal");
    }

    /// Array depth survives `SELECT *` expansion — the planner preserves the
    /// table-column refs even when the projection is implicit.
    #[test]
    fn type_info_survives_star_expansion() {
        let db = fresh_db_with_custom_types("type_info_star.db");
        let conn = db.connect_limbo();
        conn.execute("CREATE TABLE t(a INTEGER[], b INTEGER) STRICT")
            .unwrap();

        let stmt = conn.prepare("SELECT * FROM t").unwrap();

        let a_info = stmt.get_column_type_info(0).unwrap();
        assert_eq!(a_info.declared_name, "INTEGER");
        assert_eq!(a_info.array_dimensions, 1);
        assert_eq!(a_info.kind, ColumnTypeKind::Builtin);

        let b_info = stmt.get_column_type_info(1).unwrap();
        assert_eq!(b_info.declared_name, "INTEGER");
        assert_eq!(b_info.array_dimensions, 0);
        assert_eq!(b_info.kind, ColumnTypeKind::Builtin);
    }

    /// `chr(n)` is the SQL standard / PostgreSQL spelling of SQLite's
    /// `char(n)`. Both must produce identical results.
    #[test]
    fn chr_alias_matches_char() {
        let db = TempDatabase::new_empty();
        let conn = db.connect_limbo();

        let via_char: Vec<(String,)> = conn.exec_rows("SELECT char(65, 66, 67)");
        let via_chr: Vec<(String,)> = conn.exec_rows("SELECT chr(65, 66, 67)");

        assert_eq!(via_char, vec![("ABC".to_string(),)]);
        assert_eq!(via_chr, vec![("ABC".to_string(),)]);
    }

    /// `strpos(haystack, needle)` is the PostgreSQL spelling of SQLite's
    /// `instr(haystack, needle)`. Argument order is identical, so the alias
    /// is unambiguous.
    #[test]
    fn strpos_alias_matches_instr() {
        let db = TempDatabase::new_empty();
        let conn = db.connect_limbo();

        let via_instr: Vec<(i64,)> = conn.exec_rows("SELECT instr('hello world', 'world')");
        let via_strpos: Vec<(i64,)> = conn.exec_rows("SELECT strpos('hello world', 'world')");
        assert_eq!(via_instr, vec![(7,)]);
        assert_eq!(via_strpos, vec![(7,)]);

        // Needle not found → 0 from both spellings.
        let miss_instr: Vec<(i64,)> = conn.exec_rows("SELECT instr('hello', 'xyz')");
        let miss_strpos: Vec<(i64,)> = conn.exec_rows("SELECT strpos('hello', 'xyz')");
        assert_eq!(miss_instr, vec![(0,)]);
        assert_eq!(miss_strpos, vec![(0,)]);
    }
}
