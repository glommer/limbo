CREATE TABLE updatable(id INTEGER PRIMARY KEY, val TEXT);
INSERT INTO updatable VALUES (1, 'old'), (2, 'old'), (3, 'new');

CREATE MATERIALIZED VIEW count_vals AS
    SELECT val, COUNT(*) as cnt FROM updatable GROUP BY val;

SELECT 'Initial:' as phase;
SELECT val, cnt FROM count_vals ORDER BY val;

-- Simulate update by delete + insert
DELETE FROM updatable WHERE id = 1;
INSERT INTO updatable VALUES (1, 'new');

SELECT 'After update:' as phase;
SELECT val, cnt FROM count_vals ORDER BY val;

SELECT 'Base:' as phase;
SELECT id, val FROM updatable ORDER BY id;
