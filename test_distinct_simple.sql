CREATE TABLE t(a INTEGER, b INTEGER);
INSERT INTO t VALUES 
  (1, 1),
  (1, 2),
  (2, 1),
  (2, 3);

-- Test plain DISTINCT without ORDER BY (which causes issues)
CREATE MATERIALIZED VIEW v1 AS 
  SELECT DISTINCT b FROM t;

SELECT 'Plain DISTINCT b:';
SELECT * FROM v1;

-- Insert more data to see incremental updates
INSERT INTO t VALUES (3, 2), (3, 4);

SELECT 'After insert:';
SELECT * FROM v1;
