CREATE TABLE t(id INTEGER PRIMARY KEY, val REAL);
INSERT INTO t VALUES (1, 5.5), (2, 10.5), (3, 15.5);

CREATE MATERIALIZED VIEW v AS SELECT SUM(val) as sum_val FROM t;

SELECT 'Initial:';
SELECT typeof(sum_val), sum_val FROM v;

INSERT INTO t VALUES (4, 20.5);

SELECT 'After insert:';
SELECT typeof(sum_val), sum_val FROM v;
