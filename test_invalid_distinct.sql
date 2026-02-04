-- Test invalid DISTINCT + aggregate combinations
CREATE TABLE t(x INTEGER, y INTEGER);
INSERT INTO t VALUES (1, 10), (1, 10), (2, 20);

-- Try to create a view with DISTINCT and GROUP BY
-- This should either fail or be handled specially
CREATE MATERIALIZED VIEW invalid AS
    SELECT DISTINCT x, COUNT(*) as cnt FROM t GROUP BY x;

SELECT * FROM invalid;