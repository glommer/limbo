#![no_main]
use std::sync::Arc;

use libfuzzer_sys::{fuzz_target, Corpus};
use turso_core::{Database, DatabaseOpts, UnixIO};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use sql_generation::{
    generation::{Arbitrary as SqlArbitrary, GenerationContext, Opts},
    model::{
        query::{Create, Insert, Select},
        table::{Column, ColumnType, SimValue, Table},
    },
};

// Simple generation context for our fuzzer
struct FuzzContext {
    tables: Vec<Table>,
    opts: Opts,
}

impl GenerationContext for FuzzContext {
    fn tables(&self) -> &Vec<Table> {
        &self.tables
    }

    fn opts(&self) -> &Opts {
        &self.opts
    }
}

fn run_fuzzer(seed: u64) -> Corpus {
    // Use seed to create deterministic random generator
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Use a file-based database for better debugging
    let db_path = "/tmp/fuzz_materialized_views.db";
    // Always remove old database to ensure clean state
    let _ = std::fs::remove_file(db_path);
    let _ = std::fs::remove_file(format!("{}-wal", db_path));
    let _ = std::fs::remove_file(format!("{}-shm", db_path));

    let io = Arc::new(UnixIO {});
    let opts = DatabaseOpts::new().with_views(true).with_indexes(true);
    let db = match Database::open_file_with_flags(io, db_path, Default::default(), opts, None) {
        Ok(db) => db,
        Err(_) => return Corpus::Keep,
    };

    let conn = match db.connect() {
        Ok(conn) => conn,
        Err(_) => return Corpus::Keep,
    };

    // Generate a random table using sql_generation's structures
    let num_columns = rng.random_range(1..=10);
    let mut columns = Vec::new();

    for i in 0..num_columns {
        let column_type = match rng.random_range(0..3) {
            0 => ColumnType::Integer,
            1 => ColumnType::Text,
            _ => ColumnType::Float,
        };

        columns.push(Column {
            name: format!("col{}", i),
            column_type,
            primary: i == 0 && rng.random_bool(0.3),
            unique: rng.random_bool(0.1),
        });
    }

    let mut table = Table {
        name: "test_table".to_string(),
        columns,
        rows: Vec::new(), // Will populate with inserts
        indexes: Vec::new(),
    };

    // Create the table
    let create = Create { table: table.clone() };
    let create_sql = format!("{}", create);

    eprintln!("Creating table: {}", create_sql);
    if conn.prepare_execute_batch(&create_sql).is_err() {
        return Corpus::Keep;
    }

    // Set up generation context
    let mut context = FuzzContext {
        tables: vec![table.clone()],
        opts: Opts::default(),
    };

    // Generate and insert data
    let num_rows = rng.random_range(0..=1000);
    eprintln!("Inserting {} rows", num_rows);

    for _ in 0..num_rows {
        // Use sql_generation to create an INSERT
        let insert = Insert::arbitrary(&mut rng, &context);
        let insert_sql = format!("{}", insert);

        // Execute the insert
        if conn.prepare_execute_batch(&insert_sql).is_ok() {
            // If insert succeeded, add the row to our model for tracking
            // This helps sql_generation generate better queries
            if let Some(inserted_table) = context.tables.iter_mut().find(|t| t.name == insert.table()) {
                // Parse the values from the insert (simplified - just track that we have data)
                let mut row = Vec::new();
                for col in &inserted_table.columns {
                    // Generate a placeholder value based on column type
                    let value = match col.column_type {
                        ColumnType::Integer => turso_core::Value::Integer(rng.random_range(0..100)),
                        ColumnType::Float => turso_core::Value::Float(rng.random_range(0.0..100.0)),
                        ColumnType::Text => turso_core::Value::Text("test".to_string().into()),
                        ColumnType::Blob => turso_core::Value::Blob(vec![1, 2, 3]),
                    };
                    row.push(SimValue(value));
                }
                inserted_table.rows.push(row);
            }
        }
    }

    // Generate multiple SELECT queries to test with materialized views
    let num_queries = rng.random_range(5..=15);
    eprintln!("Testing {} queries", num_queries);

    let mut successful_queries = 0;

    for i in 0..num_queries {
        // Generate a random SELECT query using sql_generation
        let select = Select::arbitrary(&mut rng, &context);
        let query = format!("{}", select);

        // Only truncate for progress display
        let query_display = if query.len() > 60 {
            format!("{}...", &query[..60])
        } else {
            query.clone()
        };
        eprint!("  Query {}/{}: Testing '{}' ... ", i + 1, num_queries, query_display);

        // Create materialized view
        let view_name = format!("test_view_{}", i);
        let create_view = format!("CREATE MATERIALIZED VIEW {} AS {}", view_name, query);

        // Try to create the view - if it fails, that's OK (unsupported)
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            conn.prepare_execute_batch(&create_view)
        }));

        match result {
            Ok(Ok(_)) => {
                // View created successfully, now compare results

                // Execute query directly
                let direct_result = match conn.query(&query) {
                    Ok(Some(mut stmt)) => {
                        let mut results = Vec::new();
                        loop {
                            match stmt.step() {
                                Ok(turso_core::StepResult::Row) => {
                                    if let Some(row) = stmt.row() {
                                        let mut row_str = Vec::new();
                                        for j in 0..row.len() {
                                            let val = row.get_value(j);
                                            row_str.push(format!("{:?}", val));
                                        }
                                        results.push(row_str.join("|"));
                                    }
                                }
                                Ok(turso_core::StepResult::Done) => break,
                                Ok(turso_core::StepResult::IO) => {
                                    if stmt.run_once().is_err() {
                                        break;
                                    }
                                }
                                Ok(turso_core::StepResult::Interrupt) | Ok(turso_core::StepResult::Busy) => break,
                                Err(_) => break,
                            }
                        }
                        results.sort();
                        results
                    }
                    Ok(None) | Err(_) => {
                        eprintln!("SKIPPED (query failed)");
                        continue;
                    }
                };

                // Execute query against materialized view
                let view_query = format!("SELECT * FROM {}", view_name);
                let view_result = match conn.query(&view_query) {
                    Ok(Some(mut stmt)) => {
                        let mut results = Vec::new();
                        loop {
                            match stmt.step() {
                                Ok(turso_core::StepResult::Row) => {
                                    if let Some(row) = stmt.row() {
                                        let mut row_str = Vec::new();
                                        for j in 0..row.len() {
                                            let val = row.get_value(j);
                                            row_str.push(format!("{:?}", val));
                                        }
                                        results.push(row_str.join("|"));
                                    }
                                }
                                Ok(turso_core::StepResult::Done) => break,
                                Ok(turso_core::StepResult::IO) => {
                                    if stmt.run_once().is_err() {
                                        break;
                                    }
                                }
                                Ok(turso_core::StepResult::Interrupt) | Ok(turso_core::StepResult::Busy) => break,
                                Err(_) => break,
                            }
                        }
                        results.sort();
                        results
                    }
                    Ok(None) | Err(_) => {
                        eprintln!("SKIPPED (view query failed)");
                        continue;
                    }
                };

                // Compare results
                if direct_result != view_result {
                    // Find first difference for better debugging
                    let mut diff_info = String::new();
                    if direct_result.len() != view_result.len() {
                        diff_info.push_str(&format!("Row count mismatch: {} vs {}", direct_result.len(), view_result.len()));
                    } else {
                        for (j, (expected, actual)) in direct_result.iter().zip(view_result.iter()).enumerate() {
                            if expected != actual {
                                diff_info.push_str(&format!("Row {} differs:\n  Expected: {}\n  Got:      {}", j, expected, actual));
                                break;
                            }
                        }
                    }

                    panic!(
                        "Materialized view mismatch!\nDatabase: {}\nView: {}\nQuery: {}\n{}",
                        db_path, view_name, query, diff_info
                    );
                }

                successful_queries += 1;
                eprintln!("OK");

                // Drop the view for next iteration
                let drop_view = format!("DROP VIEW {}", view_name);
                let _ = conn.prepare_execute_batch(&drop_view);
            }
            Ok(Err(_)) => {
                eprintln!("SKIPPED (unsupported)");
                continue;
            }
            Err(panic_error) => {
                // A panic occurred - report the full query before re-panicking
                eprintln!("\nPANIC occurred while creating materialized view!");
                eprintln!("Database: {}", db_path);
                eprintln!("Full Query: {}", query);
                eprintln!("Create statement: {}", create_view);
                eprintln!("Table has {} rows", context.tables[0].rows.len());

                // Re-panic with the original error
                std::panic::resume_unwind(panic_error);
            }
        }
    }

    eprintln!("Successfully tested {}/{} queries", successful_queries, num_queries);

    Corpus::Keep
}

// The fuzzer input is just a seed for the random generator
// This way libfuzzer can still mutate seeds but we control query generation
fuzz_target!(|seed: u64| {
    run_fuzzer(seed);
});