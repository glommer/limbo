-- Test DISTINCT operator in materialized views
CREATE TABLE test_data (id INTEGER, category TEXT, value INTEGER);

-- Insert some data with duplicates
INSERT INTO test_data VALUES
    (1, 'A', 100),
    (2, 'B', 200),
    (3, 'A', 100),  -- Duplicate (A, 100)
    (4, 'C', 300),
    (5, 'B', 200),  -- Duplicate (B, 200)
    (6, 'A', 100);  -- Another duplicate (A, 100)

-- Query with DISTINCT
SELECT DISTINCT category, value FROM test_data ORDER BY category, value;
