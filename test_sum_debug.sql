CREATE TABLE t(id INTEGER PRIMARY KEY, val REAL);
INSERT INTO t VALUES (1, 5.5), (2, 10.5);

CREATE MATERIALIZED VIEW v AS SELECT SUM(val) as sum_val FROM t;

SELECT 'Initial:';
SELECT sum_val FROM v;

INSERT INTO t VALUES (3, 15.5);

SELECT 'After first insert:';
SELECT sum_val FROM v;

INSERT INTO t VALUES (4, 20.5);

SELECT 'After second insert:';
SELECT sum_val FROM v;
