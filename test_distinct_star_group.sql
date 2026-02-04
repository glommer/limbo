-- Test DISTINCT * with GROUP BY - should fail or behave unexpectedly

CREATE TABLE t(x INTEGER, y INTEGER);
INSERT INTO t VALUES (1, 10), (1, 20), (2, 30), (2, 40);

-- This is invalid in standard SQL because:
-- GROUP BY x produces one row per x value
-- But * includes y which is not in GROUP BY and not aggregated
CREATE MATERIALIZED VIEW v AS
    SELECT DISTINCT * FROM t GROUP BY x;

SELECT * FROM v;