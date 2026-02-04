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
                argv_index: Some(0),
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
            if table_name.starts_with("pragma_") || table_name.starts_with("json_") || table_name == "sqlite_dbpage" {
                continue;
            }

            let (relkind, relnatts) = match table.as_ref() {
                Table::BTree(btree_table) => {
                    ("r", btree_table.columns.len() as i64) // r = regular table
                }
                Table::Virtual(_) => {
                    ("v", 0) // v = view (virtual tables treated as views)
                }
                Table::FromClauseSubquery(_) => {
                    continue; // Skip subqueries
                }
            };

            // Create a row for this table
            self.rows.push(vec![
                Value::Integer(oid_counter),           // oid
                Value::Text(table_name.clone().into()), // relname
                Value::Integer(2200),                  // relnamespace (public schema)
                Value::Integer(0),                     // reltype
                Value::Integer(0),                     // reloftype
                Value::Integer(10),                    // relowner
                Value::Integer(0),                     // relam
                Value::Integer(0),                     // relfilenode
                Value::Integer(0),                     // reltablespace
                Value::Integer(1),                     // relpages
                Value::Float(0.0),                     // reltuples
                Value::Integer(0),                     // relallvisible
                Value::Integer(0),                     // reltoastrelid
                Value::Integer(0),                     // relhasindex
                Value::Integer(0),                     // relisshared
                Value::Text("p".into()),               // relpersistence (permanent)
                Value::Text(relkind.into()),           // relkind
                Value::Integer(relnatts),              // relnatts (number of attributes)
                Value::Integer(0),                     // relchecks
                Value::Integer(0),                     // relhasrules
                Value::Integer(0),                     // relhastriggers
                Value::Integer(0),                     // relhassubclass
                Value::Integer(0),                     // relrowsecurity
                Value::Integer(0),                     // relforcerowsecurity
                Value::Integer(1),                     // relispopulated
                Value::Text("d".into()),               // relreplident
                Value::Integer(0),                     // relispartition
                Value::Integer(0),                     // relrewrite
                Value::Integer(0),                     // relfrozenxid
                Value::Integer(0),                     // relminmxid
                Value::Null,                           // relacl
                Value::Null,                           // reloptions
                Value::Null,                           // relpartbound
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
                argv_index: Some(0),
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
    conn: Arc<Connection>,
    rows: Vec<Vec<Value>>,
    current_row: usize,
}

impl PgNamespaceCursor {
    fn new(conn: Arc<Connection>) -> Self {
        Self {
            conn,
            rows: Vec::new(),
            current_row: 0,
        }
    }

    fn load_namespaces(&mut self) -> Result<(), LimboError> {
        // PostgreSQL standard namespaces
        self.rows = vec![
            vec![
                Value::Integer(11),                    // oid
                Value::Text("pg_catalog".into()),      // nspname
                Value::Integer(10),                     // nspowner
                Value::Null,                            // nspacl
            ],
            vec![
                Value::Integer(2200),                   // oid
                Value::Text("public".into()),           // nspname
                Value::Integer(10),                     // nspowner
                Value::Null,                            // nspacl
            ],
            vec![
                Value::Integer(11394),                  // oid
                Value::Text("information_schema".into()), // nspname
                Value::Integer(10),                     // nspowner
                Value::Null,                            // nspacl
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
                argv_index: Some(0),
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
    ]
}