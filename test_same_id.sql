CREATE TABLE t(id INTEGER PRIMARY KEY, val TEXT);
INSERT INTO t VALUES (1, 'A'), (2, 'A');

CREATE MATERIALIZED VIEW v AS SELECT DISTINCT val FROM t;

SELECT 'Initial:' as phase;
SELECT val FROM v ORDER BY val;

-- Delete id=1 then reinsert with different value
DELETE FROM t WHERE id = 1;
INSERT INTO t VALUES (1, 'B');

SELECT 'After delete+insert id=1:' as phase;
SELECT val FROM v ORDER BY val;

SELECT 'Base:' as phase;
SELECT id, val FROM t ORDER BY id;
