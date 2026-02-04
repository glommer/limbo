CREATE TABLE t(val TEXT);
INSERT INTO t VALUES ('A'), ('A');

CREATE MATERIALIZED VIEW v AS SELECT DISTINCT val FROM t;

SELECT 'Initial:' as phase;
SELECT val FROM v;

-- Delete and reinsert with same rowid but different value
BEGIN;
DELETE FROM t WHERE rowid = 1;
INSERT INTO t(rowid, val) VALUES (1, 'B');
COMMIT;

SELECT 'After update rowid=1 to B:' as phase;  
SELECT val FROM v;

SELECT 'Base:' as phase;
SELECT rowid, val FROM t;
