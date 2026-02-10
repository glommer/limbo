use crate::schema::Table;
use crate::sync::{Arc, RwLock};
use crate::vtab::{InternalVirtualTable, InternalVirtualTableCursor};
use crate::{Connection, LimboError, Value};
use turso_ext::{ConstraintInfo, IndexInfo, OrderByInfo, ResultCode, VTabKind};

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
        // Query sqlite_master to get all tables, views, indexes and map to pg_class format

        // Get the schema from connection (using SQLite dialect to access sqlite_master)
        let schema = self.conn.schema.read().clone();
        self.rows.clear();

        let mut oid_counter = 16384; // Start PostgreSQL OIDs from a high number to avoid conflicts

        // Iterate through all tables in the schema
        for (table_name, table) in &schema.tables {
            // Skip SQLite system tables when in PostgreSQL mode
            if table_name == "sqlite_schema" || table_name == "sqlite_master" {
                continue;
            }

            // Skip PostgreSQL catalog tables (they have their own OIDs)
            if table_name.starts_with("pg_") {
                continue;
            }

            // Skip other SQLite-specific virtual tables
            if table_name.starts_with("pragma_")
                || table_name.starts_with("json_")
                || table_name == "sqlite_dbpage"
            {
                continue;
            }

            let (relkind, relnatts) = match table.as_ref() {
                Table::BTree(btree_table) => {
                    ("r", btree_table.columns.len() as i64) // r = regular table
                }
                Table::Virtual(_) | Table::FromClauseSubquery(_) => {
                    continue; // Skip virtual tables and subqueries
                }
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
                Value::Text(relkind.into()),            // relkind
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
    rows: Vec<Vec<Value>>,
    current_row: usize,
}

impl PgAttributeCursor {
    fn new(_conn: Arc<Connection>) -> Self {
        Self {
            rows: Vec::new(),
            current_row: 0,
        }
    }

    fn load_attributes(&mut self) -> Result<(), LimboError> {
        // This would query pragma_table_info for all tables to get column info
        // For now, return empty to test structure
        self.rows = Vec::new();
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
