-- Test mixing DISTINCT with aggregates to understand actual behavior

CREATE TABLE t(x INTEGER, y INTEGER);
INSERT INTO t VALUES (1, 10), (1, 10), (1, 20), (2, 30), (2, 30);

-- Test 1: Regular GROUP BY with COUNT (no DISTINCT)
CREATE MATERIALIZED VIEW v1 AS
    SELECT x, COUNT(*) as cnt FROM t GROUP BY x;

-- Test 2: DISTINCT with GROUP BY and COUNT
-- In SQLite, DISTINCT applies to result rows after aggregation
CREATE MATERIALIZED VIEW v2 AS
    SELECT DISTINCT x, COUNT(*) as cnt FROM t GROUP BY x;

-- Test 3: Just DISTINCT (our special case)
CREATE MATERIALIZED VIEW v3 AS
    SELECT DISTINCT x, y FROM t;

SELECT 'Regular GROUP BY:' as test;
SELECT * FROM v1 ORDER BY x;

SELECT 'DISTINCT + GROUP BY + COUNT:' as test;
SELECT * FROM v2 ORDER BY x;

SELECT 'Just DISTINCT:' as test;
SELECT * FROM v3 ORDER BY x, y;

-- Now test if updates work correctly
DELETE FROM t WHERE x = 1 AND rowid = 1;

SELECT 'After delete - Regular GROUP BY:' as test;
SELECT * FROM v1 ORDER BY x;

SELECT 'After delete - DISTINCT + GROUP BY:' as test;
SELECT * FROM v2 ORDER BY x;

SELECT 'After delete - Just DISTINCT:' as test;
SELECT * FROM v3 ORDER BY x, y;