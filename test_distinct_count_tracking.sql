-- Test that DISTINCT tracks exact counts internally
-- even though it only outputs on state transitions

CREATE TABLE t(x INTEGER);

-- Insert 5 instances of value 1
INSERT INTO t VALUES (1), (1), (1), (1), (1);

CREATE MATERIALIZED VIEW v AS SELECT DISTINCT x FROM t;

SELECT 'Initial (5 instances):' as phase;
SELECT x FROM v;

-- The view shows 1 exists (once)
-- Internally, count = 5

-- Delete 3 instances (count goes 5 -> 2)
DELETE FROM t WHERE rowid IN (1, 2, 3);

SELECT 'After deleting 3 (2 instances remain):' as phase;
SELECT x FROM v;

-- Still shows 1 exists (no output delta was generated)
-- Internally, count = 2

-- Delete 1 more (count goes 2 -> 1)
DELETE FROM t WHERE rowid = 4;

SELECT 'After deleting 1 more (1 instance remains):' as phase;
SELECT x FROM v;

-- Still shows 1 exists (no output delta was generated)
-- Internally, count = 1

-- Delete the last one (count goes 1 -> 0)
DELETE FROM t WHERE rowid = 5;

SELECT 'After deleting last (0 instances):' as phase;
SELECT x FROM v;

-- Now 1 should be gone (output delta of -1 was generated)
-- Internally, count = 0

-- Re-insert one (count goes 0 -> 1)
INSERT INTO t VALUES (1);

SELECT 'After re-insert (1 instance):' as phase;
SELECT x FROM v;

-- 1 appears again (output delta of +1 was generated)
-- Internally, count = 1

-- Insert 3 more (count goes 1 -> 4)
INSERT INTO t VALUES (1), (1), (1);

SELECT 'After inserting 3 more (4 instances):' as phase;
SELECT x FROM v;

-- Still shows 1 exists once (no output delta was generated)
-- Internally, count = 4