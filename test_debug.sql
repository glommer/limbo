CREATE TABLE t(val TEXT);
INSERT INTO t VALUES ('A'), ('A');

CREATE MATERIALIZED VIEW v AS SELECT DISTINCT val FROM t;

SELECT 'Initial:' as phase;
SELECT val FROM v;

-- This should affect group A
DELETE FROM t WHERE rowid = 1;

SELECT 'After delete rowid=1:' as phase;  
SELECT val FROM v;

-- This adds to group B, shouldn't affect A
INSERT INTO t VALUES ('B');

SELECT 'After insert B:' as phase;
SELECT val FROM v;

SELECT 'Base:' as phase;
SELECT rowid, val FROM t;
