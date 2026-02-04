-- Test that DISTINCT only outputs on zero-boundary transitions
CREATE TABLE t(x INTEGER);
INSERT INTO t VALUES (1), (1), (1);  -- Three instances of 1

CREATE MATERIALIZED VIEW v AS SELECT DISTINCT x FROM t;

SELECT 'Initial (3 instances):' as phase;
SELECT x FROM v;

-- Delete one instance - count goes from 3 to 2 (stays positive)
-- Should NOT output any change
DELETE FROM t WHERE rowid = 1;

SELECT 'After delete 1 (2 instances):' as phase;
SELECT x FROM v;

-- Delete another - count goes from 2 to 1 (stays positive)
-- Should NOT output any change
DELETE FROM t WHERE rowid = 2;

SELECT 'After delete 2 (1 instance):' as phase;
SELECT x FROM v;

-- Delete last instance - count goes from 1 to 0
-- Should output deletion
DELETE FROM t WHERE rowid = 3;

SELECT 'After delete 3 (0 instances):' as phase;
SELECT x FROM v;

-- Re-insert - count goes from 0 to 1
-- Should output insertion
INSERT INTO t VALUES (1);

SELECT 'After re-insert (1 instance):' as phase;
SELECT x FROM v;