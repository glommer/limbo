CREATE TABLE t(id INTEGER PRIMARY KEY, val);
INSERT INTO t VALUES (1, 10), (2, 20), (3, 30.5);

CREATE MATERIALIZED VIEW v AS SELECT SUM(val) as sum_val FROM t;

SELECT 'Initial with float:';
SELECT sum_val, typeof(sum_val) FROM v;

DELETE FROM t WHERE id = 3;

SELECT 'After removing the only float:';  
SELECT sum_val, typeof(sum_val) FROM v;
