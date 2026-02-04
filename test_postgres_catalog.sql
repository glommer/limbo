-- Test script for PostgreSQL catalog functionality
-- Create test tables in SQLite mode
CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);
CREATE TABLE products (id INTEGER, title TEXT, price REAL);
CREATE TABLE orders (id INTEGER, user_id INTEGER, product_id INTEGER);

-- Insert some test data
INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob');
INSERT INTO products VALUES (1, 'Widget', 9.99), (2, 'Gadget', 19.99);
INSERT INTO orders VALUES (1, 1, 1), (2, 2, 2);

-- Switch to PostgreSQL dialect
PRAGMA sql_dialect = postgres;

-- Test 1: List all user tables using pg_class
SELECT relname FROM pg_class WHERE relkind = 'r' AND relnamespace = 2200;

-- Test 2: Get all tables with details
SELECT oid, relname, relkind, relnatts
FROM pg_class
WHERE relkind = 'r' AND relnamespace = 2200;

-- Test 3: List all namespaces
SELECT * FROM pg_namespace;

-- Test 4: Check that SQLite tables are hidden
SELECT * FROM sqlite_master;

-- Test 5: Query data from a table in PostgreSQL mode
SELECT * FROM users;

-- Switch back to SQLite to verify isolation
PRAGMA sql_dialect = sqlite;
SELECT name FROM sqlite_master WHERE type = 'table';