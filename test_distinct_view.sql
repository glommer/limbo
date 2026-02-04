-- Test DISTINCT in materialized views
CREATE TABLE products (id INTEGER, category TEXT, name TEXT);

-- Create a materialized view with DISTINCT
CREATE MATERIALIZED VIEW distinct_categories AS
    SELECT DISTINCT category FROM products;

-- Initial data
INSERT INTO products VALUES
    (1, 'Electronics', 'Phone'),
    (2, 'Electronics', 'Laptop'),
    (3, 'Books', 'Novel'),
    (4, 'Electronics', 'Tablet');

-- Check the view
SELECT * FROM distinct_categories ORDER BY category;

-- Add more products with existing category
INSERT INTO products VALUES
    (5, 'Books', 'Textbook'),
    (6, 'Books', 'Magazine');

-- Check the view again (should be same)
SELECT * FROM distinct_categories ORDER BY category;

-- Add new category
INSERT INTO products VALUES (7, 'Toys', 'Puzzle');

-- Check the view (should now have 3 categories)
SELECT * FROM distinct_categories ORDER BY category;
