-- Test case: matview-insert-reused-key-maintains-all-groups
-- This test is failing after our HashableValue -> HashableRow refactoring

CREATE TABLE t(id INTEGER PRIMARY KEY, val TEXT);
INSERT INTO t VALUES (1, 'A'), (2, 'B');

CREATE MATERIALIZED VIEW v AS
    SELECT val, COUNT(*) as cnt
    FROM t
    GROUP BY val;

-- Initial state: Should show A=1, B=1
SELECT 'Initial state:';
SELECT * FROM v ORDER BY val;

-- Delete id=1 (which has 'A')
DELETE FROM t WHERE id = 1;

-- After delete: Should show only B=1 (A should be gone)
SELECT 'After DELETE:';
SELECT * FROM v ORDER BY val;

-- Insert id=1 with different value 'C'
-- This should NOT affect group 'B'
INSERT INTO t VALUES (1, 'C');

-- Final state: Should show B=1, C=1
SELECT 'Final state:';
SELECT * FROM v ORDER BY val;