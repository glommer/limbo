#!/bin/bash
cd /Users/glaubercosta/limbo

echo "=== Testing SQLite dialect ==="
echo -e "CREATE TABLE users (id INTEGER, name TEXT);\nINSERT INTO users VALUES (1, 'Alice'), (2, 'Bob');\nSELECT id, name FROM users;" | cargo run --bin tursodb -- -q :memory:

echo -e "\n\n=== Testing PostgreSQL dialect ==="
echo -e "PRAGMA sql_dialect = 'postgres';\nCREATE TABLE users (id INTEGER, name TEXT);\nINSERT INTO users VALUES (1, 'Alice'), (2, 'Bob');\nSELECT id, name FROM users;" | cargo run --bin tursodb -- -q :memory: