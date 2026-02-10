use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use async_trait::async_trait;
use futures::stream;
use tokio::net::TcpListener;
use tracing::{error, info};
use turso_core::{Connection, Value};

use pgwire::api::auth::StartupHandler;
use pgwire::api::query::SimpleQueryHandler;
use pgwire::api::results::{DataRowEncoder, FieldInfo, QueryResponse, Response, Tag};
use pgwire::api::{ClientInfo, NoopHandler, PgWireServerHandlers, Type};
use pgwire::error::{ErrorInfo, PgWireError, PgWireResult};
use pgwire::messages::data::DataRow;
use pgwire::tokio::process_socket;

pub struct TursoPgServer {
    address: String,
    conn: Arc<Mutex<Arc<Connection>>>,
    interrupt_count: Arc<AtomicUsize>,
}

impl TursoPgServer {
    pub fn new(address: String, conn: Arc<Connection>, interrupt_count: Arc<AtomicUsize>) -> Self {
        // Set postgres dialect on the connection
        conn.execute("PRAGMA sql_dialect = 'postgres'")
            .expect("failed to set postgres dialect");

        Self {
            address,
            conn: Arc::new(Mutex::new(conn)),
            interrupt_count,
        }
    }

    pub fn run(&self) -> anyhow::Result<()> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.run_async())
    }

    async fn run_async(&self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(&self.address).await?;
        info!("PostgreSQL server listening on {}", self.address);

        let factory = Arc::new(TursoPgFactory {
            handler: Arc::new(TursoPgHandler {
                conn: self.conn.clone(),
            }),
        });

        loop {
            if self.interrupt_count.load(Ordering::SeqCst) > 0 {
                info!("Shutdown signal received, stopping PostgreSQL server");
                break;
            }

            match listener.accept().await {
                Ok((socket, addr)) => {
                    info!("PostgreSQL client connected from {}", addr);
                    let factory_ref = factory.clone();
                    tokio::spawn(async move {
                        if let Err(e) = process_socket(socket, None, factory_ref).await {
                            error!("Error processing connection from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }

        Ok(())
    }
}

struct TursoPgHandler {
    conn: Arc<Mutex<Arc<Connection>>>,
}

struct TursoPgFactory {
    handler: Arc<TursoPgHandler>,
}

impl PgWireServerHandlers for TursoPgFactory {
    fn simple_query_handler(&self) -> Arc<impl SimpleQueryHandler> {
        self.handler.clone()
    }

    fn extended_query_handler(&self) -> Arc<impl pgwire::api::query::ExtendedQueryHandler> {
        Arc::new(NoopHandler)
    }

    fn startup_handler(&self) -> Arc<impl StartupHandler> {
        Arc::new(NoopHandler)
    }
}

#[async_trait]
impl SimpleQueryHandler for TursoPgHandler {
    async fn do_query<C>(&self, _client: &mut C, query: &str) -> PgWireResult<Vec<Response>>
    where
        C: ClientInfo + Unpin + Send + Sync,
    {
        let conn = self.conn.lock().unwrap().clone();

        // Try to prepare the statement
        let mut stmt = conn
            .prepare(query)
            .map_err(|e| PgWireError::UserError(Box::new(error_info(&e.to_string()))))?;

        let ncols = stmt.num_columns();

        // If no result columns, it's a non-SELECT statement (INSERT/UPDATE/DELETE/CREATE/etc.)
        if ncols == 0 {
            stmt.run_ignore_rows()
                .map_err(|e| PgWireError::UserError(Box::new(error_info(&e.to_string()))))?;

            let affected = stmt.n_change();
            let tag = command_tag(query, affected as usize);
            return Ok(vec![Response::Execution(tag)]);
        }

        // Build RowDescription from column metadata
        let fields: Vec<FieldInfo> = (0..ncols)
            .map(|i| {
                let name = stmt.get_column_name(i).into_owned();
                let pg_type = stmt
                    .get_column_decltype(i)
                    .map(|t| sqlite_type_to_pg_type(&t))
                    .unwrap_or(Type::TEXT);
                FieldInfo::new(
                    name,
                    None,
                    None,
                    pg_type,
                    pgwire::api::portal::Format::UnifiedText.format_for(i),
                )
            })
            .collect();

        let header = Arc::new(fields);

        // Collect all rows
        let mut rows: Vec<PgWireResult<DataRow>> = Vec::new();
        let header_clone = header.clone();

        stmt.run_with_row_callback(|row| {
            let mut encoder = DataRowEncoder::new(header_clone.clone());
            for val in row.get_values() {
                encode_value(&mut encoder, val)?;
            }
            rows.push(encoder.finish());
            Ok(())
        })
        .map_err(|e| PgWireError::UserError(Box::new(error_info(&e.to_string()))))?;

        let data_stream = stream::iter(rows);
        Ok(vec![Response::Query(QueryResponse::new(
            header,
            data_stream,
        ))])
    }
}

fn encode_value(encoder: &mut DataRowEncoder, val: &Value) -> turso_core::Result<()> {
    match val {
        Value::Null => encoder
            .encode_field(&None::<i8>)
            .map_err(|e| turso_core::LimboError::InternalError(e.to_string())),
        Value::Integer(i) => encoder
            .encode_field(i)
            .map_err(|e| turso_core::LimboError::InternalError(e.to_string())),
        Value::Float(f) => encoder
            .encode_field(f)
            .map_err(|e| turso_core::LimboError::InternalError(e.to_string())),
        Value::Text(t) => encoder
            .encode_field(&t.value.as_ref())
            .map_err(|e| turso_core::LimboError::InternalError(e.to_string())),
        Value::Blob(b) => encoder
            .encode_field(&b.as_slice())
            .map_err(|e| turso_core::LimboError::InternalError(e.to_string())),
    }
}

fn sqlite_type_to_pg_type(type_str: &str) -> Type {
    match type_str.to_uppercase().as_str() {
        "INTEGER" | "INT" | "INT4" | "BIGINT" | "INT8" | "SMALLINT" | "INT2" => Type::INT8,
        "REAL" | "FLOAT" | "FLOAT8" | "DOUBLE" | "DOUBLE PRECISION" | "NUMERIC" | "DECIMAL" => {
            Type::FLOAT8
        }
        "TEXT" | "VARCHAR" | "CHAR" | "CHARACTER VARYING" | "CHARACTER" => Type::TEXT,
        "BLOB" | "BYTEA" => Type::BYTEA,
        "BOOLEAN" | "BOOL" => Type::BOOL,
        _ => Type::TEXT,
    }
}

fn command_tag(query: &str, affected_rows: usize) -> Tag {
    let upper = query.trim().to_uppercase();
    if upper.starts_with("INSERT") {
        Tag::new("INSERT").with_oid(0).with_rows(affected_rows)
    } else if upper.starts_with("UPDATE") {
        Tag::new("UPDATE").with_rows(affected_rows)
    } else if upper.starts_with("DELETE") {
        Tag::new("DELETE").with_rows(affected_rows)
    } else if upper.starts_with("CREATE") {
        Tag::new("CREATE TABLE")
    } else if upper.starts_with("DROP") {
        Tag::new("DROP TABLE")
    } else if upper.starts_with("ALTER") {
        Tag::new("ALTER TABLE")
    } else {
        Tag::new("OK")
    }
}

fn error_info(message: &str) -> ErrorInfo {
    ErrorInfo::new("ERROR".to_owned(), "XX000".to_owned(), message.to_owned())
}
