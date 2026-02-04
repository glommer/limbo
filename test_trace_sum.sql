CREATE TABLE t(id INTEGER PRIMARY KEY, val REAL);

-- First batch: 5.5 + 10.5 = 16.0
INSERT INTO t VALUES (1, 5.5), (2, 10.5);
CREATE MATERIALIZED VIEW v AS SELECT SUM(val) as sum_val FROM t;
SELECT 'After first batch (5.5 + 10.5 = 16.0):';
SELECT sum_val, typeof(sum_val) FROM v;

-- Second addition: 16.0 + 15.5 = 31.5
INSERT INTO t VALUES (3, 15.5);
SELECT 'After adding 15.5 (16.0 + 15.5 = 31.5):';
SELECT sum_val, typeof(sum_val) FROM v;

-- Third addition: 31.5 + 20.5 = 52.0
INSERT INTO t VALUES (4, 20.5);
SELECT 'After adding 20.5 (31.5 + 20.5 = 52.0):';
SELECT sum_val, typeof(sum_val) FROM v;
