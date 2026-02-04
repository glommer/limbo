CREATE TABLE t(id INTEGER PRIMARY KEY, val TEXT);
INSERT INTO t VALUES (1, 'A'), (2, 'A'), (3, 'B');

CREATE MATERIALIZED VIEW v AS SELECT DISTINCT val FROM t;

SELECT 'Initial:' as phase;
SELECT val FROM v ORDER BY val;

-- Delete one A
DELETE FROM t WHERE id = 1;
SELECT 'After deleting one A:' as phase;
SELECT val FROM v ORDER BY val;

-- Add a B
INSERT INTO t VALUES (4, 'B');
SELECT 'After adding a B:' as phase;
SELECT val FROM v ORDER BY val;

-- Check the base
SELECT 'Base:' as phase;
SELECT id, val FROM t ORDER BY id;
