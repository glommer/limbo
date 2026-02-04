-- Test COUNT(DISTINCT) with incremental updates

CREATE TABLE t(grp TEXT, val INTEGER);
INSERT INTO t VALUES
    ('A', 1), ('A', 1), ('A', 2),
    ('B', 3), ('B', 3);

CREATE MATERIALIZED VIEW v AS
    SELECT grp, COUNT(DISTINCT val) as unique_vals FROM t GROUP BY grp;

SELECT 'Initial:' as phase;
SELECT * FROM v ORDER BY grp;

-- Add a new unique value to A
INSERT INTO t VALUES ('A', 4);

SELECT 'After adding unique to A:' as phase;
SELECT * FROM v ORDER BY grp;

-- Add duplicate to B (shouldn't change count)
INSERT INTO t VALUES ('B', 3);

SELECT 'After adding duplicate to B:' as phase;
SELECT * FROM v ORDER BY grp;

-- Delete one instance of duplicate in A (shouldn't change count)
DELETE FROM t WHERE grp = 'A' AND val = 1 AND rowid = 1;

SELECT 'After deleting one duplicate:' as phase;
SELECT * FROM v ORDER BY grp;

-- Delete ALL instances of value 2 in A (should decrease count)
DELETE FROM t WHERE grp = 'A' AND val = 2;

SELECT 'After deleting all of value 2:' as phase;
SELECT * FROM v ORDER BY grp;

-- Add a new group
INSERT INTO t VALUES ('C', 10), ('C', 11), ('C', 11);

SELECT 'After adding new group C:' as phase;
SELECT * FROM v ORDER BY grp;