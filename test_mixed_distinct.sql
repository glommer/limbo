-- Test if we can mix DISTINCT with other aggregates
-- In standard SQL, you can't mix DISTINCT (which operates on entire rows)
-- with other aggregates in the same SELECT

CREATE TABLE sales(region TEXT, product TEXT, amount INTEGER);
INSERT INTO sales VALUES
    ('North', 'A', 100),
    ('North', 'A', 100),  -- Duplicate
    ('North', 'B', 200),
    ('South', 'A', 150),
    ('South', 'A', 150);  -- Duplicate

-- This would be invalid SQL: can't mix SELECT DISTINCT with aggregates
-- CREATE MATERIALIZED VIEW invalid AS
--     SELECT DISTINCT region, COUNT(*) as cnt FROM sales GROUP BY region;
-- Error: Can't use DISTINCT with aggregate functions

-- You CAN use DISTINCT inside an aggregate function (COUNT DISTINCT)
-- But we don't support that yet - our DISTINCT is only for whole rows
-- CREATE MATERIALIZED VIEW count_distinct AS
--     SELECT region, COUNT(DISTINCT product) FROM sales GROUP BY region;

-- What we DO support: DISTINCT on entire result rows
CREATE MATERIALIZED VIEW v1 AS
    SELECT DISTINCT region, product, amount FROM sales;

-- And separately, regular aggregates
CREATE MATERIALIZED VIEW v2 AS
    SELECT region, COUNT(*) as cnt, SUM(amount) as total FROM sales GROUP BY region;

SELECT 'DISTINCT view:' as phase;
SELECT * FROM v1 ORDER BY region, product;

SELECT 'Aggregate view:' as phase;
SELECT * FROM v2 ORDER BY region;