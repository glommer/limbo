CREATE TABLE t(a INTEGER, b INTEGER);
INSERT INTO t VALUES 
  (1, 1),
  (1, 2),
  (2, 1),
  (2, 3);

-- Test GROUP BY with aggregate
CREATE MATERIALIZED VIEW v1 AS 
  SELECT a, COUNT(*) as cnt FROM t GROUP BY a;

SELECT 'GROUP BY a with COUNT:';
SELECT * FROM v1;

-- Now test if we can do DISTINCT on top of GROUP BY results
-- by using a subquery (if our parser doesn't accept DISTINCT...GROUP BY)
CREATE MATERIALIZED VIEW v2 AS
  SELECT DISTINCT a FROM (SELECT a, COUNT(*) FROM t GROUP BY a);

SELECT 'DISTINCT on GROUP BY results:';
SELECT * FROM v2;
