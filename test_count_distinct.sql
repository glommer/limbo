-- Test COUNT(DISTINCT x) - counting unique values

CREATE TABLE t(grp TEXT, val INTEGER);
INSERT INTO t VALUES
    ('A', 1), ('A', 1), ('A', 2), ('A', 3),
    ('B', 4), ('B', 4), ('B', 4),
    ('C', 5), ('C', 6);

-- Regular COUNT(*) for comparison
SELECT grp, COUNT(*) as total_count FROM t GROUP BY grp ORDER BY grp;

-- COUNT(DISTINCT val) - should count unique values per group
SELECT grp, COUNT(DISTINCT val) as unique_count FROM t GROUP BY grp ORDER BY grp;

-- In a materialized view
CREATE MATERIALIZED VIEW v AS
    SELECT grp, COUNT(DISTINCT val) as unique_vals FROM t GROUP BY grp;

SELECT * FROM v ORDER BY grp;