#!/bin/bash
cargo build --bin tursodb
export SQLITE_EXEC=./scripts/limbo-sqlite3
./testing/trigger.test
