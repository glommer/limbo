-- Test SUM type preservation
CREATE TABLE t(id INTEGER PRIMARY KEY, val1 INTEGER, val2 REAL);
INSERT INTO t VALUES (1, 10, 5.5), (2, 20, 10.5), (3, 30, 15.5);

CREATE MATERIALIZED VIEW v AS
    SELECT 
        SUM(val1) as sum_int,  -- Should be INTEGER
        SUM(val2) as sum_real, -- Should be REAL
        SUM(val1 + val2) as sum_mixed -- Should be REAL
    FROM t;

-- Check initial state 
SELECT 'Initial SUM types:';
SELECT typeof(sum_int), typeof(sum_real), typeof(sum_mixed) FROM v;
SELECT sum_int, sum_real, sum_mixed FROM v;

-- Add more integer values
INSERT INTO t VALUES (4, 40, 20.5);

-- Check types are preserved
SELECT 'After insert:';
SELECT typeof(sum_int), typeof(sum_real), typeof(sum_mixed) FROM v;
SELECT sum_int, sum_real, sum_mixed FROM v;

-- Delete a row
DELETE FROM t WHERE id = 2;

-- Check types are still preserved
SELECT 'After delete:';
SELECT typeof(sum_int), typeof(sum_real), typeof(sum_mixed) FROM v;
SELECT sum_int, sum_real, sum_mixed FROM v;
