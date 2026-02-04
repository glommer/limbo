CREATE TABLE test (id INTEGER, value TEXT);
INSERT INTO test VALUES (1, '100'), (2, '200');

-- First, explain without CAST
EXPLAIN SELECT * FROM test WHERE value > '150';

.print "\n--- With CAST ---"
-- Now explain with CAST
EXPLAIN SELECT * FROM test WHERE CAST(value AS INTEGER) > 150;

.print "\n--- With CAST in condition ---"
-- Another variant
EXPLAIN SELECT * FROM test WHERE id < CAST('5' AS INTEGER);
