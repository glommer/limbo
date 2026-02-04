-- Test that COUNT(*) and COUNT(DISTINCT) can be used together
-- This should work without any issues

DROP TABLE IF EXISTS orders;
CREATE TABLE orders (
    id INTEGER PRIMARY KEY,
    customer_id INTEGER,
    product_id INTEGER,
    amount REAL,
    status TEXT
);

-- Insert test data with duplicates
INSERT INTO orders VALUES
    (1, 100, 1, 50.0, 'completed'),
    (2, 100, 2, 75.0, 'completed'),
    (3, 100, 1, 50.0, 'completed'),  -- Same customer, same product
    (4, 200, 1, 50.0, 'completed'),
    (5, 200, 3, 100.0, 'pending'),
    (6, 300, 2, 75.0, 'completed'),
    (7, 300, 2, 75.0, 'completed'),  -- Same customer, same product
    (8, 400, 4, 125.0, 'completed'),
    (9, 400, 4, 125.0, 'pending'),   -- Same customer, same product
    (10, 500, 1, 50.0, 'completed');

-- Create view with both COUNT(*) and COUNT(DISTINCT)
DROP VIEW IF EXISTS order_summary;
CREATE VIEW order_summary AS
SELECT
    status,
    COUNT(*) as total_orders,
    COUNT(DISTINCT customer_id) as unique_customers,
    COUNT(DISTINCT product_id) as unique_products,
    SUM(amount) as total_amount,
    SUM(DISTINCT amount) as sum_distinct_amounts
FROM orders
GROUP BY status;

-- Check initial state
SELECT '=== Initial State ===' as phase;
SELECT * FROM order_summary ORDER BY status;

-- Add more orders to test incremental updates
SELECT '=== After Insert ===' as phase;
INSERT INTO orders VALUES
    (11, 100, 5, 200.0, 'completed'),  -- Existing customer, new product, new amount
    (12, 600, 1, 50.0, 'pending'),     -- New customer, existing product, existing amount
    (13, 300, 2, 75.0, 'completed');   -- Existing customer, existing product, existing amount

SELECT * FROM order_summary ORDER BY status;

-- Delete some orders
SELECT '=== After Delete ===' as phase;
DELETE FROM orders WHERE id IN (1, 2, 3);

SELECT * FROM order_summary ORDER BY status;

-- Update an order (delete + insert)
SELECT '=== After Update ===' as phase;
UPDATE orders SET amount = 150.0, status = 'pending' WHERE id = 4;

SELECT * FROM order_summary ORDER BY status;

-- Final verification with raw data
SELECT '=== Raw Data ===' as phase;
SELECT * FROM orders ORDER BY status, customer_id, product_id;

SELECT '=== Final Summary ===' as phase;
SELECT * FROM order_summary ORDER BY status;