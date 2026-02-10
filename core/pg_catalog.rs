use crate::schema::{Schema, Table};
use crate::sync::{Arc, RwLock};
use crate::vtab::{InternalVirtualTable, InternalVirtualTableCursor};
use crate::{Connection, LimboError, Value};
use turso_ext::{ConstraintInfo, IndexInfo, OrderByInfo, ResultCode, VTabKind};

/// Starting OID for user tables (matches PostgreSQL convention)
const USER_TABLE_OID_START: i64 = 16384;

/// Returns an iterator of (table_name, table_ref) for user tables in deterministic order.
/// Both pg_class and pg_attribute must use this function to ensure consistent OID assignment.
fn user_tables_sorted(schema: &Schema) -> Vec<(&String, &Arc<Table>)> {
    let mut tables: Vec<_> = schema
        .tables
        .iter()
        .filter(|(name, table)| {
            // Skip system tables
            if *name == "sqlite_schema"
                || *name == "sqlite_master"
                || name.starts_with("pg_")
                || name.starts_with("pragma_")
                || name.starts_with("json_")
                || *name == "sqlite_dbpage"
            {
                return false;
            }
            // Skip virtual tables and subqueries
            matches!(table.as_ref(), Table::BTree(_))
        })
        .collect();
    tables.sort_by_key(|(name, _)| *name);
    tables
}

/// Map a SQLite type string to a PostgreSQL type OID.
fn sqlite_type_to_pg_oid(ty_str: &str) -> i64 {
    match ty_str.to_uppercase().as_str() {
        "INTEGER" | "INT" | "SMALLINT" | "BIGINT" | "TINYINT" | "MEDIUMINT" => 23, // int4
        "TEXT" | "VARCHAR" | "CHAR" | "CLOB" | "NCHAR" | "NVARCHAR" => 25,         // text
        "REAL" | "DOUBLE" | "DOUBLE PRECISION" | "FLOAT" => 701,                   // float8
        "BLOB" => 17,                                                               // bytea
        "NUMERIC" | "DECIMAL" => 1700,                                              // numeric
        "BOOLEAN" | "BOOL" => 16,                                                   // bool
        _ => 25,                                                                    // default to text
    }
}

/// Virtual table implementation for pg_catalog.pg_class
/// Maps SQLite's sqlite_master to PostgreSQL's pg_class system table
#[derive(Debug)]
pub struct PgClassTable;

impl PgClassTable {
    pub fn new() -> Self {
        Self
    }
}

impl InternalVirtualTable for PgClassTable {
    fn name(&self) -> String {
        "pg_class".to_string()
    }

    fn open(
        &self,
        conn: Arc<Connection>,
    ) -> crate::Result<Arc<RwLock<dyn InternalVirtualTableCursor>>> {
        Ok(Arc::new(RwLock::new(PgClassCursor::new(conn))))
    }

    fn best_index(
        &self,
        constraints: &[ConstraintInfo],
        _order_by: &[OrderByInfo],
    ) -> Result<IndexInfo, ResultCode> {
        // Create constraint usages for each constraint
        let constraint_usages = constraints
            .iter()
            .map(|_constraint| turso_ext::ConstraintUsage {
                argv_index: None, // We'll handle filtering ourselves
                omit: false,
            })
            .collect();

        Ok(IndexInfo {
            idx_num: 0,
            idx_str: None,
            order_by_consumed: false,
            estimated_cost: 1000.0,
            estimated_rows: 100,
            constraint_usages,
        })
    }

    fn sql(&self) -> String {
        // PostgreSQL pg_class columns (simplified subset)
        "CREATE TABLE pg_class (
            oid INTEGER,
            relname TEXT,
            relnamespace INTEGER,
            reltype INTEGER,
            reloftype INTEGER,
            relowner INTEGER,
            relam INTEGER,
            relfilenode INTEGER,
            reltablespace INTEGER,
            relpages INTEGER,
            reltuples REAL,
            relallvisible INTEGER,
            reltoastrelid INTEGER,
            relhasindex INTEGER,
            relisshared INTEGER,
            relpersistence TEXT,
            relkind TEXT,
            relnatts INTEGER,
            relchecks INTEGER,
            relhasrules INTEGER,
            relhastriggers INTEGER,
            relhassubclass INTEGER,
            relrowsecurity INTEGER,
            relforcerowsecurity INTEGER,
            relispopulated INTEGER,
            relreplident TEXT,
            relispartition INTEGER,
            relrewrite INTEGER,
            relfrozenxid INTEGER,
            relminmxid INTEGER,
            relacl TEXT,
            reloptions TEXT,
            relpartbound TEXT
        )"
        .to_string()
    }
}

struct PgClassCursor {
    conn: Arc<Connection>,
    rows: Vec<Vec<Value>>,
    current_row: usize,
}

impl PgClassCursor {
    fn new(conn: Arc<Connection>) -> Self {
        Self {
            conn,
            rows: Vec::new(),
            current_row: 0,
        }
    }

    fn load_from_sqlite_master(&mut self) -> Result<(), LimboError> {
        let schema = self.conn.schema.read().clone();
        self.rows.clear();

        let mut oid_counter = USER_TABLE_OID_START;

        for (table_name, table) in user_tables_sorted(&schema) {
            let relnatts = match table.as_ref() {
                Table::BTree(btree_table) => btree_table.columns.len() as i64,
                _ => continue,
            };

            // Create a row for this table
            self.rows.push(vec![
                Value::Integer(oid_counter),            // oid
                Value::Text(table_name.clone().into()), // relname
                Value::Integer(2200),                   // relnamespace (public schema)
                Value::Integer(0),                      // reltype
                Value::Integer(0),                      // reloftype
                Value::Integer(10),                     // relowner
                Value::Integer(0),                      // relam
                Value::Integer(0),                      // relfilenode
                Value::Integer(0),                      // reltablespace
                Value::Integer(1),                      // relpages
                Value::Float(0.0),                      // reltuples
                Value::Integer(0),                      // relallvisible
                Value::Integer(0),                      // reltoastrelid
                Value::Integer(0),                      // relhasindex
                Value::Integer(0),                      // relisshared
                Value::Text("p".into()),                // relpersistence (permanent)
                Value::Text("r".into()),                // relkind (regular table)
                Value::Integer(relnatts),               // relnatts (number of attributes)
                Value::Integer(0),                      // relchecks
                Value::Integer(0),                      // relhasrules
                Value::Integer(0),                      // relhastriggers
                Value::Integer(0),                      // relhassubclass
                Value::Integer(0),                      // relrowsecurity
                Value::Integer(0),                      // relforcerowsecurity
                Value::Integer(1),                      // relispopulated
                Value::Text("d".into()),                // relreplident
                Value::Integer(0),                      // relispartition
                Value::Integer(0),                      // relrewrite
                Value::Integer(0),                      // relfrozenxid
                Value::Integer(0),                      // relminmxid
                Value::Null,                            // relacl
                Value::Null,                            // reloptions
                Value::Null,                            // relpartbound
            ]);

            oid_counter += 1;
        }

        Ok(())
    }
}

impl InternalVirtualTableCursor for PgClassCursor {
    fn next(&mut self) -> Result<bool, LimboError> {
        self.current_row += 1;
        Ok(self.current_row < self.rows.len())
    }

    fn rowid(&self) -> i64 {
        self.current_row as i64
    }

    fn column(&self, column: usize) -> Result<Value, LimboError> {
        if self.current_row < self.rows.len() && column < self.rows[self.current_row].len() {
            Ok(self.rows[self.current_row][column].clone())
        } else {
            Ok(Value::Null)
        }
    }

    fn filter(
        &mut self,
        _args: &[Value],
        _idx_str: Option<String>,
        _idx_num: i32,
    ) -> Result<bool, LimboError> {
        // Reset cursor and load data
        self.current_row = 0;
        self.rows.clear();
        self.load_from_sqlite_master()?;

        // Return true if we have any rows
        Ok(!self.rows.is_empty())
    }
}

/// Virtual table implementation for pg_catalog.pg_namespace
/// Maps schema information to PostgreSQL's pg_namespace
#[derive(Debug)]
pub struct PgNamespaceTable;

impl PgNamespaceTable {
    pub fn new() -> Self {
        Self
    }
}

impl InternalVirtualTable for PgNamespaceTable {
    fn name(&self) -> String {
        "pg_namespace".to_string()
    }

    fn open(
        &self,
        conn: Arc<Connection>,
    ) -> crate::Result<Arc<RwLock<dyn InternalVirtualTableCursor>>> {
        Ok(Arc::new(RwLock::new(PgNamespaceCursor::new(conn))))
    }

    fn best_index(
        &self,
        constraints: &[ConstraintInfo],
        _order_by: &[OrderByInfo],
    ) -> Result<IndexInfo, ResultCode> {
        let constraint_usages = constraints
            .iter()
            .map(|_constraint| turso_ext::ConstraintUsage {
                argv_index: None, // We'll handle filtering ourselves
                omit: false,
            })
            .collect();

        Ok(IndexInfo {
            idx_num: 0,
            idx_str: None,
            order_by_consumed: false,
            estimated_cost: 10.0,
            estimated_rows: 5,
            constraint_usages,
        })
    }

    fn sql(&self) -> String {
        "CREATE TABLE pg_namespace (
            oid INTEGER,
            nspname TEXT,
            nspowner INTEGER,
            nspacl TEXT
        )"
        .to_string()
    }
}

struct PgNamespaceCursor {
    rows: Vec<Vec<Value>>,
    current_row: usize,
}

impl PgNamespaceCursor {
    fn new(_conn: Arc<Connection>) -> Self {
        Self {
            rows: Vec::new(),
            current_row: 0,
        }
    }

    fn load_namespaces(&mut self) -> Result<(), LimboError> {
        // PostgreSQL standard namespaces
        self.rows = vec![
            vec![
                Value::Integer(11),               // oid
                Value::Text("pg_catalog".into()), // nspname
                Value::Integer(10),               // nspowner
                Value::Null,                      // nspacl
            ],
            vec![
                Value::Integer(2200),         // oid
                Value::Text("public".into()), // nspname
                Value::Integer(10),           // nspowner
                Value::Null,                  // nspacl
            ],
            vec![
                Value::Integer(11394),                    // oid
                Value::Text("information_schema".into()), // nspname
                Value::Integer(10),                       // nspowner
                Value::Null,                              // nspacl
            ],
        ];
        Ok(())
    }
}

impl InternalVirtualTableCursor for PgNamespaceCursor {
    fn next(&mut self) -> Result<bool, LimboError> {
        self.current_row += 1;
        Ok(self.current_row < self.rows.len())
    }

    fn rowid(&self) -> i64 {
        self.current_row as i64
    }

    fn column(&self, column: usize) -> Result<Value, LimboError> {
        if self.current_row < self.rows.len() && column < self.rows[self.current_row].len() {
            Ok(self.rows[self.current_row][column].clone())
        } else {
            Ok(Value::Null)
        }
    }

    fn filter(
        &mut self,
        _args: &[Value],
        _idx_str: Option<String>,
        _idx_num: i32,
    ) -> Result<bool, LimboError> {
        self.current_row = 0;
        self.rows.clear();
        self.load_namespaces()?;
        Ok(!self.rows.is_empty())
    }
}

/// Virtual table implementation for pg_catalog.pg_attribute
/// Maps column information to PostgreSQL's pg_attribute
#[derive(Debug)]
pub struct PgAttributeTable;

impl PgAttributeTable {
    pub fn new() -> Self {
        Self
    }
}

impl InternalVirtualTable for PgAttributeTable {
    fn name(&self) -> String {
        "pg_attribute".to_string()
    }

    fn open(
        &self,
        conn: Arc<Connection>,
    ) -> crate::Result<Arc<RwLock<dyn InternalVirtualTableCursor>>> {
        Ok(Arc::new(RwLock::new(PgAttributeCursor::new(conn))))
    }

    fn best_index(
        &self,
        constraints: &[ConstraintInfo],
        _order_by: &[OrderByInfo],
    ) -> Result<IndexInfo, ResultCode> {
        let constraint_usages = constraints
            .iter()
            .map(|_constraint| turso_ext::ConstraintUsage {
                argv_index: None, // We'll handle filtering ourselves
                omit: false,
            })
            .collect();

        Ok(IndexInfo {
            idx_num: 0,
            idx_str: None,
            order_by_consumed: false,
            estimated_cost: 1000.0,
            estimated_rows: 1000,
            constraint_usages,
        })
    }

    fn sql(&self) -> String {
        "CREATE TABLE pg_attribute (
            attrelid INTEGER,
            attname TEXT,
            atttypid INTEGER,
            attstattarget INTEGER,
            attlen INTEGER,
            attnum INTEGER,
            attndims INTEGER,
            attcacheoff INTEGER,
            atttypmod INTEGER,
            attbyval INTEGER,
            attstorage TEXT,
            attalign TEXT,
            attnotnull INTEGER,
            atthasdef INTEGER,
            atthasmissing INTEGER,
            attidentity TEXT,
            attgenerated TEXT,
            attisdropped INTEGER,
            attislocal INTEGER,
            attinhcount INTEGER,
            attcollation INTEGER,
            attacl TEXT,
            attoptions TEXT,
            attfdwoptions TEXT,
            attmissingval TEXT
        )"
        .to_string()
    }
}

struct PgAttributeCursor {
    conn: Arc<Connection>,
    rows: Vec<Vec<Value>>,
    current_row: usize,
}

impl PgAttributeCursor {
    fn new(conn: Arc<Connection>) -> Self {
        Self {
            conn,
            rows: Vec::new(),
            current_row: 0,
        }
    }

    fn load_attributes(&mut self) -> Result<(), LimboError> {
        let schema = self.conn.schema.read().clone();
        self.rows.clear();

        let mut oid_counter = USER_TABLE_OID_START;

        for (_, table) in user_tables_sorted(&schema) {
            let table_oid = oid_counter;
            oid_counter += 1;

            let columns = table.columns();
            for (i, col) in columns.iter().enumerate() {
                let col_name = col.name.clone().unwrap_or_default();
                let type_oid = sqlite_type_to_pg_oid(&col.ty_str);
                let attnum = (i + 1) as i64; // 1-based
                let notnull = if col.notnull() { 1i64 } else { 0i64 };
                let has_def = if col.default.is_some() { 1i64 } else { 0i64 };

                self.rows.push(vec![
                    Value::Integer(table_oid),            // attrelid
                    Value::Text(col_name.into()),         // attname
                    Value::Integer(type_oid),              // atttypid
                    Value::Integer(-1),                    // attstattarget
                    Value::Integer(-1),                    // attlen
                    Value::Integer(attnum),                // attnum
                    Value::Integer(0),                     // attndims
                    Value::Integer(-1),                    // attcacheoff
                    Value::Integer(-1),                    // atttypmod
                    Value::Integer(1),                     // attbyval
                    Value::Text("p".into()),               // attstorage (plain)
                    Value::Text("i".into()),               // attalign (int)
                    Value::Integer(notnull),               // attnotnull
                    Value::Integer(has_def),               // atthasdef
                    Value::Integer(0),                     // atthasmissing
                    Value::Text("".into()),                // attidentity
                    Value::Text("".into()),                // attgenerated
                    Value::Integer(0),                     // attisdropped
                    Value::Integer(1),                     // attislocal
                    Value::Integer(0),                     // attinhcount
                    Value::Integer(0),                     // attcollation
                    Value::Null,                           // attacl
                    Value::Null,                           // attoptions
                    Value::Null,                           // attfdwoptions
                    Value::Null,                           // attmissingval
                ]);
            }
        }

        Ok(())
    }
}

impl InternalVirtualTableCursor for PgAttributeCursor {
    fn next(&mut self) -> Result<bool, LimboError> {
        self.current_row += 1;
        Ok(self.current_row < self.rows.len())
    }

    fn rowid(&self) -> i64 {
        self.current_row as i64
    }

    fn column(&self, column: usize) -> Result<Value, LimboError> {
        if self.current_row < self.rows.len() && column < self.rows[self.current_row].len() {
            Ok(self.rows[self.current_row][column].clone())
        } else {
            Ok(Value::Null)
        }
    }

    fn filter(
        &mut self,
        _args: &[Value],
        _idx_str: Option<String>,
        _idx_num: i32,
    ) -> Result<bool, LimboError> {
        self.current_row = 0;
        self.rows.clear();
        self.load_attributes()?;
        Ok(!self.rows.is_empty())
    }
}

/// Virtual table implementation for pg_catalog.pg_roles
/// Stub: returns a single hardcoded "turso" superuser role.
/// TODO: replace with real role data when authentication is implemented.
#[derive(Debug)]
pub struct PgRolesTable;

impl PgRolesTable {
    pub fn new() -> Self {
        Self
    }

    /// Stub: returns a single default superuser role.
    /// Replace this method with real role lookup when auth is implemented.
    fn roles() -> Vec<Vec<Value>> {
        vec![vec![
            Value::Integer(10),              // oid
            Value::build_text("turso"),      // rolname
            Value::Integer(1),               // rolsuper
            Value::Integer(1),               // rolinherit
            Value::Integer(1),               // rolcreaterole
            Value::Integer(1),               // rolcreatedb
            Value::Integer(1),               // rolcanlogin
            Value::Integer(1),               // rolreplication
            Value::Integer(-1),              // rolconnlimit (-1 = no limit)
            Value::Null,                     // rolpassword (never exposed)
            Value::Null,                     // rolvaliduntil
            Value::Integer(1),               // rolbypassrls
            Value::Null,                     // rolconfig
        ]]
    }
}

impl InternalVirtualTable for PgRolesTable {
    fn name(&self) -> String {
        "pg_roles".to_string()
    }

    fn open(
        &self,
        _conn: Arc<Connection>,
    ) -> crate::Result<Arc<RwLock<dyn InternalVirtualTableCursor>>> {
        Ok(Arc::new(RwLock::new(PgRolesCursor {
            rows: Vec::new(),
            current_row: 0,
        })))
    }

    fn best_index(
        &self,
        constraints: &[ConstraintInfo],
        _order_by: &[OrderByInfo],
    ) -> Result<IndexInfo, ResultCode> {
        let constraint_usages = constraints
            .iter()
            .map(|_| turso_ext::ConstraintUsage {
                argv_index: None,
                omit: false,
            })
            .collect();

        Ok(IndexInfo {
            idx_num: 0,
            idx_str: None,
            order_by_consumed: false,
            estimated_cost: 10.0,
            estimated_rows: 1,
            constraint_usages,
        })
    }

    fn sql(&self) -> String {
        "CREATE TABLE pg_roles (
            oid INTEGER,
            rolname TEXT,
            rolsuper INTEGER,
            rolinherit INTEGER,
            rolcreaterole INTEGER,
            rolcreatedb INTEGER,
            rolcanlogin INTEGER,
            rolreplication INTEGER,
            rolconnlimit INTEGER,
            rolpassword TEXT,
            rolvaliduntil TEXT,
            rolbypassrls INTEGER,
            rolconfig TEXT
        )"
        .to_string()
    }
}

struct PgRolesCursor {
    rows: Vec<Vec<Value>>,
    current_row: usize,
}

impl InternalVirtualTableCursor for PgRolesCursor {
    fn next(&mut self) -> Result<bool, LimboError> {
        self.current_row += 1;
        Ok(self.current_row < self.rows.len())
    }

    fn rowid(&self) -> i64 {
        self.current_row as i64
    }

    fn column(&self, column: usize) -> Result<Value, LimboError> {
        if self.current_row < self.rows.len() && column < self.rows[self.current_row].len() {
            Ok(self.rows[self.current_row][column].clone())
        } else {
            Ok(Value::Null)
        }
    }

    fn filter(
        &mut self,
        _args: &[Value],
        _idx_str: Option<String>,
        _idx_num: i32,
    ) -> Result<bool, LimboError> {
        self.current_row = 0;
        self.rows = PgRolesTable::roles();
        Ok(!self.rows.is_empty())
    }
}

/// Virtual table implementation for pg_catalog.pg_am
/// Stub: returns two access methods (heap and btree).
#[derive(Debug)]
pub struct PgAmTable;

impl PgAmTable {
    pub fn new() -> Self {
        Self
    }

    fn rows() -> Vec<Vec<Value>> {
        vec![
            vec![
                Value::Integer(2),                // oid
                Value::build_text("heap"),        // amname
                Value::build_text("heap_tableam_handler"), // amhandler
                Value::build_text("t"),           // amtype (table)
            ],
            vec![
                Value::Integer(403),              // oid
                Value::build_text("btree"),       // amname
                Value::build_text("bthandler"),   // amhandler
                Value::build_text("i"),           // amtype (index)
            ],
        ]
    }
}

impl InternalVirtualTable for PgAmTable {
    fn name(&self) -> String {
        "pg_am".to_string()
    }

    fn open(
        &self,
        _conn: Arc<Connection>,
    ) -> crate::Result<Arc<RwLock<dyn InternalVirtualTableCursor>>> {
        Ok(Arc::new(RwLock::new(PgAmCursor {
            rows: Vec::new(),
            current_row: 0,
        })))
    }

    fn best_index(
        &self,
        constraints: &[ConstraintInfo],
        _order_by: &[OrderByInfo],
    ) -> Result<IndexInfo, ResultCode> {
        let constraint_usages = constraints
            .iter()
            .map(|_| turso_ext::ConstraintUsage {
                argv_index: None,
                omit: false,
            })
            .collect();

        Ok(IndexInfo {
            idx_num: 0,
            idx_str: None,
            order_by_consumed: false,
            estimated_cost: 10.0,
            estimated_rows: 2,
            constraint_usages,
        })
    }

    fn sql(&self) -> String {
        "CREATE TABLE pg_am (
            oid INTEGER,
            amname TEXT,
            amhandler TEXT,
            amtype TEXT
        )"
        .to_string()
    }
}

struct PgAmCursor {
    rows: Vec<Vec<Value>>,
    current_row: usize,
}

impl InternalVirtualTableCursor for PgAmCursor {
    fn next(&mut self) -> Result<bool, LimboError> {
        self.current_row += 1;
        Ok(self.current_row < self.rows.len())
    }

    fn rowid(&self) -> i64 {
        self.current_row as i64
    }

    fn column(&self, column: usize) -> Result<Value, LimboError> {
        if self.current_row < self.rows.len() && column < self.rows[self.current_row].len() {
            Ok(self.rows[self.current_row][column].clone())
        } else {
            Ok(Value::Null)
        }
    }

    fn filter(
        &mut self,
        _args: &[Value],
        _idx_str: Option<String>,
        _idx_num: i32,
    ) -> Result<bool, LimboError> {
        self.current_row = 0;
        self.rows = PgAmTable::rows();
        Ok(!self.rows.is_empty())
    }
}

/// Generic empty PG catalog table — always returns no rows.
/// Used for catalog tables psql queries but we don't yet need real data for.
#[derive(Debug)]
struct EmptyPgCatalogTable {
    name: String,
    create_sql: String,
}

impl InternalVirtualTable for EmptyPgCatalogTable {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn open(
        &self,
        _conn: Arc<Connection>,
    ) -> crate::Result<Arc<RwLock<dyn InternalVirtualTableCursor>>> {
        Ok(Arc::new(RwLock::new(EmptyPgCatalogCursor)))
    }

    fn best_index(
        &self,
        constraints: &[ConstraintInfo],
        _order_by: &[OrderByInfo],
    ) -> Result<IndexInfo, ResultCode> {
        let constraint_usages = constraints
            .iter()
            .map(|_| turso_ext::ConstraintUsage {
                argv_index: None,
                omit: false,
            })
            .collect();
        Ok(IndexInfo {
            idx_num: 0,
            idx_str: None,
            order_by_consumed: false,
            estimated_cost: 10.0,
            estimated_rows: 0,
            constraint_usages,
        })
    }

    fn sql(&self) -> String {
        self.create_sql.clone()
    }
}

struct EmptyPgCatalogCursor;

impl InternalVirtualTableCursor for EmptyPgCatalogCursor {
    fn next(&mut self) -> Result<bool, LimboError> {
        Ok(false)
    }
    fn rowid(&self) -> i64 {
        0
    }
    fn column(&self, _column: usize) -> Result<Value, LimboError> {
        Ok(Value::Null)
    }
    fn filter(
        &mut self,
        _args: &[Value],
        _idx_str: Option<String>,
        _idx_num: i32,
    ) -> Result<bool, LimboError> {
        Ok(false)
    }
}

fn empty_catalog_table(name: &str, create_sql: &str) -> Arc<crate::vtab::VirtualTable> {
    use crate::vtab::VirtualTable;
    let table = EmptyPgCatalogTable {
        name: name.to_string(),
        create_sql: create_sql.to_string(),
    };
    Arc::new(
        VirtualTable::new_internal(
            name.to_string(),
            table.sql(),
            VTabKind::VirtualTable,
            Arc::new(RwLock::new(table)),
        )
        .unwrap_or_else(|_| panic!("{name} virtual table creation should not fail")),
    )
}

/// Create PostgreSQL system catalog virtual tables
pub fn pg_catalog_virtual_tables() -> Vec<Arc<crate::vtab::VirtualTable>> {
    use crate::vtab::VirtualTable;

    vec![
        // pg_class virtual table
        Arc::new(
            VirtualTable::new_internal(
                "pg_class".to_string(),
                PgClassTable::new().sql(),
                VTabKind::VirtualTable,
                Arc::new(RwLock::new(PgClassTable::new())),
            )
            .expect("pg_class virtual table creation should not fail"),
        ),
        // pg_namespace virtual table
        Arc::new(
            VirtualTable::new_internal(
                "pg_namespace".to_string(),
                PgNamespaceTable::new().sql(),
                VTabKind::VirtualTable,
                Arc::new(RwLock::new(PgNamespaceTable::new())),
            )
            .expect("pg_namespace virtual table creation should not fail"),
        ),
        // pg_attribute virtual table
        Arc::new(
            VirtualTable::new_internal(
                "pg_attribute".to_string(),
                PgAttributeTable::new().sql(),
                VTabKind::VirtualTable,
                Arc::new(RwLock::new(PgAttributeTable::new())),
            )
            .expect("pg_attribute virtual table creation should not fail"),
        ),
        // pg_roles virtual table
        Arc::new(
            VirtualTable::new_internal(
                "pg_roles".to_string(),
                PgRolesTable::new().sql(),
                VTabKind::VirtualTable,
                Arc::new(RwLock::new(PgRolesTable::new())),
            )
            .expect("pg_roles virtual table creation should not fail"),
        ),
        // pg_am virtual table
        Arc::new(
            VirtualTable::new_internal(
                "pg_am".to_string(),
                PgAmTable::new().sql(),
                VTabKind::VirtualTable,
                Arc::new(RwLock::new(PgAmTable::new())),
            )
            .expect("pg_am virtual table creation should not fail"),
        ),
        // pg_get_tabledef virtual table (custom extension for getting PostgreSQL DDL)
        Arc::new(
            VirtualTable::new_internal(
                "pg_get_tabledef".to_string(),
                PgGetTableDefTable::new().sql(),
                VTabKind::VirtualTable,
                Arc::new(RwLock::new(PgGetTableDefTable::new())),
            )
            .expect("pg_get_tabledef virtual table creation should not fail"),
        ),
        // Empty stub tables for psql \d command compatibility
        empty_catalog_table("pg_policy", "CREATE TABLE pg_policy (oid INTEGER, polname TEXT, polpermissive TEXT, polroles TEXT, polcmd TEXT, polqual TEXT, polwithcheck TEXT, polrelid INTEGER)"),
        empty_catalog_table("pg_trigger", "CREATE TABLE pg_trigger (oid INTEGER, tgrelid INTEGER, tgname TEXT, tgfoid INTEGER, tgtype INTEGER, tgenabled TEXT, tgisinternal INTEGER, tgconstrrelid INTEGER, tgconstrindid INTEGER, tgconstraint INTEGER, tgdeferrable INTEGER, tginitdeferred INTEGER, tgnargs INTEGER, tgattr TEXT, tgargs TEXT, tgqual TEXT, tgoldtable TEXT, tgnewtable TEXT)"),
        empty_catalog_table("pg_index", "CREATE TABLE pg_index (indexrelid INTEGER, indrelid INTEGER, indnatts INTEGER, indnkeyatts INTEGER, indisunique INTEGER, indisprimary INTEGER, indisexclusion INTEGER, indimmediate INTEGER, indisclustered INTEGER, indisvalid INTEGER, indcheckxmin INTEGER, indisready INTEGER, indislive INTEGER, indisreplident INTEGER, indkey TEXT, indcollation TEXT, indclass TEXT, indoption TEXT, indexprs TEXT, indpred TEXT)"),
        empty_catalog_table("pg_constraint", "CREATE TABLE pg_constraint (oid INTEGER, conname TEXT, connamespace INTEGER, contype TEXT, condeferrable INTEGER, condeferred INTEGER, convalidated INTEGER, conrelid INTEGER, contypid INTEGER, conindid INTEGER, conparentid INTEGER, confrelid INTEGER, confupdtype TEXT, confdeltype TEXT, confmatchtype TEXT, conislocal INTEGER, coninhcount INTEGER, connoinherit INTEGER, conkey TEXT, confkey TEXT, conpfeqop TEXT, conppeqop TEXT, conffeqop TEXT, conexclop TEXT, conbin TEXT)"),
        empty_catalog_table("pg_statistic_ext", "CREATE TABLE pg_statistic_ext (oid INTEGER, stxrelid INTEGER, stxname TEXT, stxnamespace INTEGER, stxowner INTEGER, stxstattarget INTEGER, stxkeys TEXT, stxkind TEXT, stxexprs TEXT)"),
        empty_catalog_table("pg_inherits", "CREATE TABLE pg_inherits (inhrelid INTEGER, inhparent INTEGER, inhseqno INTEGER, inhdetachpending INTEGER)"),
        empty_catalog_table("pg_rewrite", "CREATE TABLE pg_rewrite (oid INTEGER, rulename TEXT, ev_class INTEGER, ev_type TEXT, ev_enabled TEXT, is_instead INTEGER, ev_qual TEXT, ev_action TEXT)"),
        empty_catalog_table("pg_foreign_table", "CREATE TABLE pg_foreign_table (ftrelid INTEGER, ftserver INTEGER, ftoptions TEXT)"),
        empty_catalog_table("pg_partitioned_table", "CREATE TABLE pg_partitioned_table (partrelid INTEGER, partstrat TEXT, partnatts INTEGER, partdefid INTEGER, partattrs TEXT, partclass TEXT, partcollation TEXT, partexprs TEXT)"),
        empty_catalog_table("pg_type", "CREATE TABLE pg_type (oid INTEGER, typname TEXT, typnamespace INTEGER, typowner INTEGER, typlen INTEGER, typbyval INTEGER, typtype TEXT, typcategory TEXT, typispreferred INTEGER, typisdefined INTEGER, typdelim TEXT, typrelid INTEGER, typsubscript TEXT, typelem INTEGER, typarray INTEGER, typinput TEXT, typoutput TEXT, typreceive TEXT, typsend TEXT, typmodin TEXT, typmodout TEXT, typanalyze TEXT, typalign TEXT, typstorage TEXT, typnotnull INTEGER, typbasetype INTEGER, typtypmod INTEGER, typndims INTEGER, typcollation INTEGER, typdefaultbin TEXT, typdefault TEXT, typacl TEXT)"),
        empty_catalog_table("pg_collation", "CREATE TABLE pg_collation (oid INTEGER, collname TEXT, collnamespace INTEGER, collowner INTEGER, collprovider TEXT, collisdeterministic INTEGER, collencoding INTEGER, collcollate TEXT, collctype TEXT, colliculocale TEXT, collicurules TEXT, collversion TEXT)"),
        empty_catalog_table("pg_attrdef", "CREATE TABLE pg_attrdef (oid INTEGER, adrelid INTEGER, adnum INTEGER, adbin TEXT)"),
        empty_catalog_table("pg_description", "CREATE TABLE pg_description (objoid INTEGER, classoid INTEGER, objsubid INTEGER, description TEXT)"),
        empty_catalog_table("pg_publication", "CREATE TABLE pg_publication (oid INTEGER, pubname TEXT, pubowner INTEGER, puballtables INTEGER, pubinsert INTEGER, pubupdate INTEGER, pubdelete INTEGER, pubtruncate INTEGER, pubviaroot INTEGER)"),
        empty_catalog_table("pg_publication_namespace", "CREATE TABLE pg_publication_namespace (oid INTEGER, pnpubid INTEGER, pnnspid INTEGER)"),
        empty_catalog_table("pg_publication_rel", "CREATE TABLE pg_publication_rel (oid INTEGER, prpubid INTEGER, prrelid INTEGER, prqual TEXT, prattrs TEXT)"),
    ]
}

/// Virtual table for getting PostgreSQL-compatible CREATE TABLE statements
#[derive(Debug)]
struct PgGetTableDefTable;

impl PgGetTableDefTable {
    fn new() -> Self {
        Self
    }
}

struct PgGetTableDefCursor {
    conn: Arc<Connection>,
    rows: Vec<Vec<Value>>,
    current_row: usize,
    row_count: usize,
}

impl PgGetTableDefCursor {
    fn new(conn: Arc<Connection>) -> Self {
        Self {
            conn,
            rows: Vec::new(),
            current_row: 0,
            row_count: 0,
        }
    }

    fn load_table_defs(&mut self) -> Result<(), LimboError> {
        // Get schema with read lock
        let schema = self.conn.schema.read().clone();
        self.rows.clear();

        // Get DDL from sqlite_master for each user table
        for (table_name, table) in &schema.tables {
            // Skip system tables (sqlite_master, sqlite_schema, etc.)
            if table_name.starts_with("sqlite_")
                || table_name == "sqlite_master"
                || table_name == "sqlite_schema"
            {
                continue;
            }

            // Skip virtual tables and subqueries
            if !matches!(table.as_ref(), Table::BTree(_)) {
                continue;
            }

            // Get the original SQLite DDL and convert to PostgreSQL
            let sqlite_ddl = self.get_sqlite_ddl(table_name)?;
            let postgres_ddl = self.convert_to_postgres_ddl(&sqlite_ddl);

            self.rows.push(vec![
                Value::Text("public".into()), // schema_name (PostgreSQL default)
                Value::Text(table_name.clone().into()),
                Value::Text(postgres_ddl.into()),
            ]);
        }

        Ok(())
    }

    fn get_sqlite_ddl(&self, table_name: &str) -> Result<String, LimboError> {
        // Get the DDL from the schema's sqlite_master info
        // For now, we'll reconstruct it from the table definition
        // In a full implementation, we'd query sqlite_master directly
        let schema = self.conn.schema.read();

        if let Some(table) = schema.tables.get(table_name) {
            if let Table::BTree(btree_table) = table.as_ref() {
                let mut ddl = format!("CREATE TABLE {table_name} (");
                let cols: Vec<String> = btree_table
                    .columns
                    .iter()
                    .map(|col| {
                        let col_name = col.name.as_deref().unwrap_or("unnamed");
                        let ty_str = &col.ty_str;
                        let mut col_def = format!("{col_name} {ty_str}");

                        // Check if this column is a primary key
                        for (pk_col, _) in &btree_table.primary_key_columns {
                            if pk_col == col_name {
                                col_def.push_str(" PRIMARY KEY");
                                break;
                            }
                        }

                        // Add NOT NULL if column is not nullable
                        if col.notnull() {
                            col_def.push_str(" NOT NULL");
                        }

                        // Add default value if present
                        if col.default.is_some() {
                            col_def.push_str(" DEFAULT ..."); // Simplified for now
                        }

                        col_def
                    })
                    .collect();
                ddl.push_str(&cols.join(", "));
                ddl.push(')');
                return Ok(ddl);
            }
        }
        Ok(format!("CREATE TABLE {table_name} (...)"))
    }

    fn convert_to_postgres_ddl(&self, sqlite_ddl: &str) -> String {
        let mut postgres_ddl = sqlite_ddl.to_string();

        // Basic SQLite to PostgreSQL type conversions
        // Handle INTEGER PRIMARY KEY specially for SERIAL
        postgres_ddl = postgres_ddl.replace(" INTEGER PRIMARY KEY", " SERIAL PRIMARY KEY");
        postgres_ddl = postgres_ddl.replace(" AUTOINCREMENT", "");

        // Type conversions - use lowercase for PostgreSQL standard
        // Use regex-like replacements to handle case-insensitive matches
        let type_replacements = [
            (" INTEGER", " integer"),
            (" intEgEr", " integer"),
            (" REAL", " double precision"),
            (" real", " double precision"),
            (" TEXT", " text"),
            (" text", " text"),
            (" BLOB", " bytea"),
            (" blob", " bytea"),
            (" DATETIME", " timestamp"),
            (" datetime", " timestamp"),
        ];

        for (from, to) in &type_replacements {
            postgres_ddl = postgres_ddl.replace(from, to);
        }

        // Remove SQLite-specific features
        postgres_ddl = postgres_ddl.replace(" WITHOUT ROWID", "");

        postgres_ddl
    }
}

impl InternalVirtualTableCursor for PgGetTableDefCursor {
    fn next(&mut self) -> Result<bool, LimboError> {
        self.current_row += 1;
        Ok(self.current_row < self.row_count)
    }

    fn rowid(&self) -> i64 {
        self.current_row as i64
    }

    fn column(&self, column: usize) -> Result<Value, LimboError> {
        if self.current_row < self.rows.len() && column < 3 {
            Ok(self.rows[self.current_row][column].clone())
        } else {
            Ok(Value::Null)
        }
    }

    fn filter(
        &mut self,
        _args: &[Value],
        _idx_str: Option<String>,
        _idx_num: i32,
    ) -> Result<bool, LimboError> {
        self.current_row = 0;
        self.load_table_defs()?;
        self.row_count = self.rows.len();
        Ok(!self.rows.is_empty())
    }
}

impl InternalVirtualTable for PgGetTableDefTable {
    fn name(&self) -> String {
        "pg_get_tabledef".to_string()
    }

    fn sql(&self) -> String {
        "CREATE TABLE pg_get_tabledef (
            schema_name TEXT,
            table_name TEXT,
            ddl TEXT
        )"
        .to_string()
    }

    fn open(
        &self,
        conn: Arc<Connection>,
    ) -> crate::Result<Arc<RwLock<dyn InternalVirtualTableCursor>>> {
        Ok(Arc::new(RwLock::new(PgGetTableDefCursor::new(conn))))
    }

    fn best_index(
        &self,
        _constraints: &[ConstraintInfo],
        _order_by: &[OrderByInfo],
    ) -> Result<IndexInfo, ResultCode> {
        Ok(IndexInfo {
            idx_num: 0,
            idx_str: None,
            order_by_consumed: false,
            estimated_cost: 100.0,
            estimated_rows: 20,
            constraint_usages: vec![],
        })
    }
}

// TODO: Fix tests to use correct API
#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use super::*;
    use crate::{Database, PlatformIO, StepResult};
    use tempfile::tempdir;

    #[test]

    fn test_pg_namespace_query() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let io = Arc::new(PlatformIO::new().unwrap());
        let db = Database::open_file_with_flags(
            io,
            db_path.to_str().unwrap(),
            crate::OpenFlags::default(),
            crate::DatabaseOpts::new().with_postgres(true),
            None,
        )
        .unwrap();
        let conn = db.connect().unwrap();

        // Switch to PostgreSQL dialect
        conn.execute("PRAGMA sql_dialect = 'postgres'").unwrap();

        // Query pg_namespace
        let mut stmt = conn.prepare("SELECT * FROM pg_namespace").unwrap();

        let mut found_pg_catalog = false;
        let mut found_public = false;
        let mut found_information_schema = false;

        loop {
            match stmt.step().unwrap() {
                StepResult::Row => {
                    let row = stmt.row().unwrap();
                    if let Value::Text(nspname) = row.get_value(1) {
                        match nspname.value.as_ref() {
                            "pg_catalog" => found_pg_catalog = true,
                            "public" => found_public = true,
                            "information_schema" => found_information_schema = true,
                            _ => {}
                        }
                    }
                }
                StepResult::Done => break,
                _ => {}
            }
        }

        assert!(found_pg_catalog, "pg_catalog namespace not found");
        assert!(found_public, "public namespace not found");
        assert!(
            found_information_schema,
            "information_schema namespace not found"
        );
    }

    #[test]

    fn test_pg_class_lists_user_tables() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let io = Arc::new(PlatformIO::new().unwrap());
        let db = Database::open_file_with_flags(
            io,
            db_path.to_str().unwrap(),
            crate::OpenFlags::default(),
            crate::DatabaseOpts::new().with_postgres(true),
            None,
        )
        .unwrap();
        let conn = db.connect().unwrap();

        // Create test tables in SQLite mode (default)
        conn.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")
            .unwrap();
        conn.execute("CREATE TABLE products (id INTEGER, title TEXT, price REAL)")
            .unwrap();
        conn.execute("CREATE TABLE orders (id INTEGER, user_id INTEGER, product_id INTEGER)")
            .unwrap();

        // Switch to PostgreSQL dialect
        conn.execute("PRAGMA sql_dialect = 'postgres'").unwrap();

        // Query pg_class for regular tables
        let mut stmt = conn
            .prepare("SELECT relname FROM pg_class WHERE relkind = 'r' AND relnamespace = 2200")
            .unwrap();

        let mut tables = Vec::new();
        loop {
            match stmt.step().unwrap() {
                StepResult::Row => {
                    let row = stmt.row().unwrap();
                    if let Value::Text(relname) = row.get_value(0) {
                        tables.push(relname.to_string());
                    }
                }
                StepResult::Done => break,
                _ => {}
            }
        }

        // Should find our three tables
        assert!(
            tables.contains(&"users".to_string()),
            "users table not found"
        );
        assert!(
            tables.contains(&"products".to_string()),
            "products table not found"
        );
        assert!(
            tables.contains(&"orders".to_string()),
            "orders table not found"
        );
        assert_eq!(tables.len(), 3, "Expected exactly 3 tables");
    }

    #[test]

    fn test_pg_class_table_details() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let io = Arc::new(PlatformIO::new().unwrap());
        let db = Database::open_file_with_flags(
            io,
            db_path.to_str().unwrap(),
            crate::OpenFlags::default(),
            crate::DatabaseOpts::new().with_postgres(true),
            None,
        )
        .unwrap();
        let conn = db.connect().unwrap();

        // Create a test table with known columns
        conn.execute("CREATE TABLE test_table (id INTEGER, name TEXT, value REAL)")
            .unwrap();

        // Switch to PostgreSQL dialect
        conn.execute("PRAGMA sql_dialect = 'postgres'").unwrap();

        // Query pg_class for table details
        let mut stmt = conn
            .prepare(
                "SELECT oid, relname, relkind, relnatts
             FROM pg_class
             WHERE relname = 'test_table'",
            )
            .unwrap();

        if let StepResult::Row = stmt.step().unwrap() {
            let row = stmt.row().unwrap();
            let oid = if let Value::Integer(v) = row.get_value(0) {
                *v
            } else {
                panic!("Expected OID")
            };
            let relname = if let Value::Text(v) = row.get_value(1) {
                v
            } else {
                panic!("Expected relname")
            };
            let relkind = if let Value::Text(v) = row.get_value(2) {
                v
            } else {
                panic!("Expected relkind")
            };
            let relnatts = if let Value::Integer(v) = row.get_value(3) {
                *v
            } else {
                panic!("Expected relnatts")
            };

            assert!(oid >= 16384, "OID should be >= 16384 for user tables");
            assert_eq!(relname.value, "test_table", "Table name should match");
            assert_eq!(
                relkind.value, "r",
                "relkind should be 'r' for regular table"
            );
            assert_eq!(relnatts, 3, "Table should have 3 columns");
        } else {
            panic!("test_table not found in pg_class");
        }
    }

    #[test]

    fn test_sqlite_tables_hidden_in_postgres_mode() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let io = Arc::new(PlatformIO::new().unwrap());
        let db = Database::open_file_with_flags(
            io,
            db_path.to_str().unwrap(),
            crate::OpenFlags::default(),
            crate::DatabaseOpts::new().with_postgres(true),
            None,
        )
        .unwrap();
        let conn = db.connect().unwrap();

        // Create a test table
        conn.execute("CREATE TABLE test_table (id INTEGER)")
            .unwrap();

        // Switch to PostgreSQL dialect
        conn.execute("PRAGMA sql_dialect = 'postgres'").unwrap();

        // Try to query sqlite_master - should fail
        let result = conn.prepare("SELECT * FROM sqlite_master");
        assert!(
            result.is_err(),
            "sqlite_master should not be accessible in PostgreSQL mode"
        );

        // Try to query sqlite_schema - should also fail
        let result = conn.prepare("SELECT * FROM sqlite_schema");
        assert!(
            result.is_err(),
            "sqlite_schema should not be accessible in PostgreSQL mode"
        );
    }

    #[test]

    fn test_postgres_tables_hidden_in_sqlite_mode() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let io = Arc::new(PlatformIO::new().unwrap());
        let db = Database::open_file_with_flags(
            io,
            db_path.to_str().unwrap(),
            crate::OpenFlags::default(),
            crate::DatabaseOpts::new().with_postgres(true),
            None,
        )
        .unwrap();
        let conn = db.connect().unwrap();

        // Default is SQLite mode

        // Try to query pg_class - should fail
        let result = conn.prepare("SELECT * FROM pg_class");
        assert!(
            result.is_err(),
            "pg_class should not be accessible in SQLite mode"
        );

        // Try to query pg_namespace - should fail
        let result = conn.prepare("SELECT * FROM pg_namespace");
        assert!(
            result.is_err(),
            "pg_namespace should not be accessible in SQLite mode"
        );

        // sqlite_master should work
        let result = conn.prepare("SELECT * FROM sqlite_master");
        assert!(
            result.is_ok(),
            "sqlite_master should be accessible in SQLite mode"
        );
    }

    #[test]

    fn test_dialect_switching() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let io = Arc::new(PlatformIO::new().unwrap());
        let db = Database::open_file_with_flags(
            io,
            db_path.to_str().unwrap(),
            crate::OpenFlags::default(),
            crate::DatabaseOpts::new().with_postgres(true),
            None,
        )
        .unwrap();
        let conn = db.connect().unwrap();

        // Create a test table
        conn.execute("CREATE TABLE users (id INTEGER, name TEXT)")
            .unwrap();

        // In SQLite mode, check sqlite_master
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
            .unwrap();
        let mut found = false;
        loop {
            match stmt.step().unwrap() {
                StepResult::Row => {
                    let row = stmt.row().unwrap();
                    if let Value::Text(name) = row.get_value(0) {
                        if name.value == "users" {
                            found = true;
                        }
                    }
                }
                StepResult::Done => break,
                _ => {}
            }
        }
        assert!(found, "users table not found in sqlite_master");

        // Switch to PostgreSQL mode
        conn.execute("PRAGMA sql_dialect = 'postgres'").unwrap();

        // In PostgreSQL mode, check pg_class
        let mut stmt = conn
            .prepare("SELECT relname FROM pg_class WHERE relkind = 'r'")
            .unwrap();
        let mut found = false;
        loop {
            match stmt.step().unwrap() {
                StepResult::Row => {
                    let row = stmt.row().unwrap();
                    if let Value::Text(name) = row.get_value(0) {
                        if name.value == "users" {
                            found = true;
                        }
                    }
                }
                StepResult::Done => break,
                _ => {}
            }
        }
        assert!(found, "users table not found in pg_class");

        // Switch back to SQLite mode
        conn.execute("PRAGMA sql_dialect = 'sqlite'").unwrap();

        // sqlite_master should work again
        let result = conn.prepare("SELECT * FROM sqlite_master");
        assert!(
            result.is_ok(),
            "sqlite_master should be accessible after switching back to SQLite mode"
        );
    }

    #[test]

    fn test_pg_class_with_where_constraints() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let io = Arc::new(PlatformIO::new().unwrap());
        let db = Database::open_file_with_flags(
            io,
            db_path.to_str().unwrap(),
            crate::OpenFlags::default(),
            crate::DatabaseOpts::new().with_postgres(true),
            None,
        )
        .unwrap();
        let conn = db.connect().unwrap();

        // Create multiple tables
        conn.execute("CREATE TABLE table1 (id INTEGER)").unwrap();
        conn.execute("CREATE TABLE table2 (id INTEGER, name TEXT)")
            .unwrap();
        conn.execute("CREATE TABLE table3 (id INTEGER, name TEXT, value REAL)")
            .unwrap();

        // Switch to PostgreSQL dialect
        conn.execute("PRAGMA sql_dialect = 'postgres'").unwrap();

        // Test various WHERE clause combinations

        // Test 1: Filter by relkind = 'r'
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM pg_class WHERE relkind = 'r'")
            .unwrap();
        match stmt.step().unwrap() {
            StepResult::Row => {
                let row = stmt.row().unwrap();
                if let Value::Integer(count) = row.get_value(0) {
                    assert_eq!(*count, 3, "Should have 3 regular tables");
                }
            }
            _ => panic!("Expected row from COUNT query"),
        }

        // Test 2: Filter by relnamespace = 2200 (public schema)
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM pg_class WHERE relnamespace = 2200")
            .unwrap();
        match stmt.step().unwrap() {
            StepResult::Row => {
                let row = stmt.row().unwrap();
                if let Value::Integer(count) = row.get_value(0) {
                    assert_eq!(*count, 3, "Should have 3 tables in public schema");
                }
            }
            _ => panic!("Expected row from COUNT query"),
        }

        // Test 3: Combined filters
        let mut stmt = conn.prepare("SELECT relname FROM pg_class WHERE relkind = 'r' AND relnamespace = 2200 ORDER BY relname").unwrap();
        let mut tables = Vec::new();
        loop {
            match stmt.step().unwrap() {
                StepResult::Row => {
                    let row = stmt.row().unwrap();
                    if let Value::Text(name) = row.get_value(0) {
                        tables.push(name.to_string());
                    }
                }
                StepResult::Done => break,
                _ => {}
            }
        }
        assert_eq!(tables, vec!["table1", "table2", "table3"]);
    }
}
