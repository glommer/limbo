CREATE TABLE test (
    id INTEGER,
    value TEXT,
    amount REAL
);

INSERT INTO test VALUES (1, 'abc', 100.0);
INSERT INTO test VALUES (2, 'def', 200.0);
INSERT INTO test VALUES (3, 'xyz', 300.0);

-- Test with hex() function in WHERE clause
CREATE MATERIALIZED VIEW test_hex AS
SELECT id, amount
FROM test
WHERE hex(id) = '31';  -- hex(1) = '31' 

SELECT * FROM test_hex;

-- Test with upper() function
CREATE MATERIALIZED VIEW test_upper AS
SELECT id, amount  
FROM test
WHERE upper(value) = 'ABC';

SELECT * FROM test_upper;

-- Test with length() function
CREATE MATERIALIZED VIEW test_length AS
SELECT id, amount
FROM test  
WHERE length(value) > 2;

SELECT * FROM test_length;
