-- Test PostgreSQL dialect with real tables

-- Create a table using SQLite dialect (default)
CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER);
INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30);
INSERT INTO users (id, name, age) VALUES (2, 'Bob', 25);

-- Switch to PostgreSQL dialect
PRAGMA sql_dialect = postgres;

-- Try to read from the table using PostgreSQL parser
SELECT * FROM users;
SELECT id, name FROM users;
SELECT name FROM users WHERE id = 1;