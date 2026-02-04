# Sort/Limit Operator Issue

## Problem

The SortLimitOperator correctly computes sorted top-k results incrementally, but when the data is persisted and read back, it returns in hash order instead of sorted order.

## Root Cause

### Writing (commit phase)
In `sort_limit_operator.rs:540-543`, rows are written to storage with:
```rust
let element_hash = row.cached_hash();  // Hash based on row content
let index_key = vec![
    Value::Integer(storage_id),
    zset_id.to_value(),
    element_hash.to_value(),  // THIS IS THE PROBLEM
];
```

The `element_hash` is computed from row content, not from sort position.

### Reading (query phase)
In `cursor.rs`, `MaterializedViewCursor` reads from the btree in whatever order the btree returns rows - which is ordered by the index key (storage_id, zset_id, element_hash).

Since element_hash is based on content hash, rows come back in hash order, NOT sort order.

## Example

```sql
CREATE TABLE scores(id INTEGER, points INTEGER);
INSERT INTO scores VALUES (1, 100), (2, 85), (3, 95);
CREATE MATERIALIZED VIEW v AS SELECT * FROM scores ORDER BY points ASC;
SELECT * FROM v;
```

Expected: 2|85, 3|95, 1|100 (sorted by points)
Actual: Random order based on hash(row content)

## Solution Options

### Option 1: Store sort position in element_hash (Recommended)
For SortLimitOperator, use a sequence number as element_hash instead of content hash:
```rust
// In WriteNewState, iterate with index:
for (position, (row, weight)) in new_rows.iter().enumerate() {
    let element_hash = Hash128::new(0, position as u64);  // Sort position
    ...
}
```

Pros:
- Preserves sort order naturally
- Simple to implement
- Works with existing cursor logic

Cons:
- Different from other operators (they use content hash)
- Need to handle updates carefully

### Option 2: Post-sort in MaterializedViewCursor
Add sort logic to the cursor when reading from a view with ORDER BY.

Pros:
- Keeps operator storage consistent
- Centralizes sorting logic

Cons:
- More complex
- Performance overhead on every read
- Need to pass ORDER BY info to cursor

### Option 3: Separate index for sort order
Create a secondary index on (storage_id, sort_position) -> rowid.

Pros:
- Most flexible
- Can support multiple sort orders

Cons:
- Most complex
- Additional storage overhead

## Recommendation

Implement Option 1 for now:
1. Modify SortLimitOperator to use sequence-based element_hash
2. Ensure sequence is stable across updates (difficult part!)
3. Document the approach

The key challenge is maintaining stable positions when rows are inserted/deleted from the top-k.

## Test Status

Tests in `testing/materialized_views.test` starting at line ~2502 are currently failing due to this issue. They should pass once the fix is implemented.
