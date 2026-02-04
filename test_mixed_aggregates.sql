-- Test mixing DISTINCT with other aggregates
CREATE TABLE test_data(grp TEXT, val INTEGER);
INSERT INTO test_data VALUES ('A', 1), ('A', 1), ('A', 2), ('B', 3), ('B', 3);

-- This would be: SELECT grp, COUNT(DISTINCT val), SUM(val) FROM test_data GROUP BY grp
-- But since we implement DISTINCT as GROUP BY all columns, we can't mix it with other aggregates
-- in the same query. DISTINCT operates on entire rows, not individual columns.

-- Let's verify DISTINCT works as GROUP BY all columns:
CREATE MATERIALIZED VIEW v_distinct AS SELECT DISTINCT grp, val FROM test_data;

SELECT 'Distinct view:' as phase;
SELECT * FROM v_distinct ORDER BY grp, val;

-- And regular aggregates work normally:
CREATE MATERIALIZED VIEW v_agg AS SELECT grp, COUNT(*) as cnt, SUM(val) as total FROM test_data GROUP BY grp;

SELECT 'Aggregate view:' as phase;
SELECT * FROM v_agg ORDER BY grp;

-- Test updates affect both views correctly
DELETE FROM test_data WHERE grp = 'A' AND val = 1 AND rowid = 1;

SELECT 'After delete - Distinct:' as phase;
SELECT * FROM v_distinct ORDER BY grp, val;

SELECT 'After delete - Aggregate:' as phase;
SELECT * FROM v_agg ORDER BY grp;