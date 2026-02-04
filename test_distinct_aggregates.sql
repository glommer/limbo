-- Test file for distinct aggregates in materialized views
-- This tests COUNT(DISTINCT), AVG(DISTINCT), and SUM(DISTINCT) on different columns

-- Create base table
DROP TABLE IF EXISTS sales;
CREATE TABLE sales (
    id INTEGER PRIMARY KEY,
    product_id INTEGER,
    quantity INTEGER,
    price REAL,
    category TEXT
);

-- Insert test data with duplicates to test DISTINCT
INSERT INTO sales VALUES
    -- Product 10 appears multiple times
    (1, 10, 5, 100.0, 'Electronics'),
    (2, 10, 3, 100.0, 'Electronics'),
    (3, 10, 7, 100.0, 'Electronics'),

    -- Product 20 appears multiple times
    (4, 20, 2, 50.0, 'Books'),
    (5, 20, 4, 50.0, 'Books'),

    -- Product 30 appears once
    (6, 30, 1, 75.0, 'Clothing'),

    -- Product 40 appears multiple times with different quantities/prices
    (7, 40, 5, 200.0, 'Electronics'),
    (8, 40, 5, 200.0, 'Electronics'),
    (9, 40, 8, 200.0, 'Electronics'),

    -- More diverse data
    (10, 50, 2, 25.0, 'Books'),
    (11, 60, 3, 150.0, 'Electronics'),
    (12, 70, 1, 300.0, 'Electronics');

-- Create materialized view with distinct aggregates on different columns
DROP VIEW IF EXISTS sales_summary;
CREATE VIEW sales_summary AS
SELECT
    category,
    COUNT(DISTINCT product_id) as unique_products,
    AVG(DISTINCT quantity) as avg_distinct_quantity,
    SUM(DISTINCT price) as total_distinct_prices
FROM sales
GROUP BY category;

-- Initial state
SELECT '=== Initial View State ===' as phase;
SELECT * FROM sales_summary ORDER BY category;

-- Test incremental updates
SELECT '=== After Insert ===' as phase;
INSERT INTO sales VALUES
    (13, 10, 5, 100.0, 'Electronics'),  -- Duplicate product_id, quantity, price
    (14, 80, 10, 400.0, 'Electronics'),  -- New product with new values
    (15, 20, 2, 60.0, 'Books');          -- Existing product, same quantity, new price

SELECT * FROM sales_summary ORDER BY category;

-- Test deletions
SELECT '=== After Delete ===' as phase;
DELETE FROM sales WHERE id IN (1, 2, 3);  -- Remove all instances of product 10 in Electronics

SELECT * FROM sales_summary ORDER BY category;

-- Test updates (implemented as delete + insert)
SELECT '=== After Update ===' as phase;
UPDATE sales SET quantity = 15, price = 500.0 WHERE id = 7;  -- Change product 40's values

SELECT * FROM sales_summary ORDER BY category;

-- Final verification
SELECT '=== Final State ===' as phase;
SELECT * FROM sales_summary ORDER BY category;

-- Show raw data for verification
SELECT '=== Raw Data ===' as phase;
SELECT * FROM sales ORDER BY category, product_id;