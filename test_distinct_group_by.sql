CREATE TABLE t(category TEXT, item TEXT, price INTEGER);
INSERT INTO t VALUES 
  ('fruit', 'apple', 10),
  ('fruit', 'apple', 15),
  ('fruit', 'banana', 20),
  ('veg', 'carrot', 5),
  ('veg', 'carrot', 8),
  ('veg', 'potato', 12);

-- Test 1: DISTINCT with GROUP BY on different column
CREATE MATERIALIZED VIEW v1 AS 
  SELECT DISTINCT item FROM t GROUP BY category ORDER BY item;

-- Test 2: Just GROUP BY (for comparison)
CREATE MATERIALIZED VIEW v2 AS
  SELECT item FROM t GROUP BY category ORDER BY item;

-- Test 3: Plain DISTINCT
CREATE MATERIALIZED VIEW v3 AS
  SELECT DISTINCT item FROM t ORDER BY item;

SELECT 'Test 1 - DISTINCT item GROUP BY category:';
SELECT * FROM v1;

SELECT 'Test 2 - Just GROUP BY category:';
SELECT * FROM v2;

SELECT 'Test 3 - Plain DISTINCT item:';
SELECT * FROM v3;
