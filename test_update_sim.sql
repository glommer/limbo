CREATE TABLE updatable(id INTEGER PRIMARY KEY, val TEXT);
INSERT INTO updatable VALUES (1, 'old'), (2, 'old'), (3, 'new');

CREATE MATERIALIZED VIEW distinct_vals AS
    SELECT DISTINCT val FROM updatable;

-- Check initial state
SELECT 'Initial:' as phase;
SELECT val FROM distinct_vals ORDER BY val;

-- Simulate update by delete + insert
DELETE FROM updatable WHERE id = 1;

SELECT 'After delete:' as phase;
SELECT val FROM distinct_vals ORDER BY val;

INSERT INTO updatable VALUES (1, 'new');

SELECT 'After insert:' as phase;
SELECT val FROM distinct_vals ORDER BY val;

-- Check base table
SELECT 'Base table:' as phase;
SELECT id, val FROM updatable ORDER BY id;
