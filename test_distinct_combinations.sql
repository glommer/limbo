-- Test various DISTINCT + GROUP BY combinations

CREATE TABLE t(x INTEGER, y INTEGER, z INTEGER);
INSERT INTO t VALUES
    (1, 10, 100),
    (1, 10, 100),  -- Exact duplicate
    (1, 20, 200),
    (2, 30, 300),
    (2, 30, 300);  -- Exact duplicate

-- Test 1: DISTINCT with all columns in GROUP BY
-- This should work - DISTINCT is redundant but harmless
CREATE MATERIALIZED VIEW v1 AS
    SELECT DISTINCT x, y FROM t GROUP BY x, y;

-- Test 2: DISTINCT with subset of columns not in GROUP BY
-- This should work - y is selected but not grouped
CREATE MATERIALIZED VIEW v2 AS
    SELECT DISTINCT x, COUNT(*) as cnt, SUM(z) as total
    FROM t GROUP BY x;

-- Test 3: Plain DISTINCT (no GROUP BY)
CREATE MATERIALIZED VIEW v3 AS
    SELECT DISTINCT x, y FROM t;

SELECT 'Test 1 - DISTINCT x,y GROUP BY x,y:' as test;
SELECT * FROM v1 ORDER BY x, y;

SELECT 'Test 2 - DISTINCT with aggregates:' as test;
SELECT * FROM v2 ORDER BY x;

SELECT 'Test 3 - Plain DISTINCT:' as test;
SELECT * FROM v3 ORDER BY x, y;