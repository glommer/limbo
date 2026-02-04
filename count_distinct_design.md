# COUNT(DISTINCT) Implementation Design

## Problem
`SELECT grp, COUNT(DISTINCT val) FROM t GROUP BY grp`

Currently broken in materialized views - ignores DISTINCT and just counts all rows.

## Solution: Chain Two Operators

### Stage 1: Distinct Aggregator
- GROUP BY: [grp, val]
- Aggregates: [Distinct] (our special aggregate that tracks existence)
- Input: (grp, val, other_cols...)
- Output: (grp, val) - only unique combinations

### Stage 2: Count Aggregator
- GROUP BY: [grp]
- Aggregates: [Count]
- Input: (grp, val) from stage 1
- Output: (grp, count)

## Example Flow

Input data:
```
('A', 1)
('A', 1)
('A', 2)
('B', 3)
('B', 3)
```

After Stage 1 (DISTINCT on grp,val):
```
('A', 1) weight=1
('A', 2) weight=1
('B', 3) weight=1
```

After Stage 2 (COUNT by grp):
```
('A', 2)  # counted 2 distinct values
('B', 1)  # counted 1 distinct value
```

## Implementation Notes

When we detect `COUNT(DISTINCT col)` in the compiler:
1. Check if the aggregate has `distinct: true`
2. Create a first aggregator that groups by (original_groups + distinct_column)
3. Feed its output to a second aggregator that groups by original_groups and counts

This generalizes to other DISTINCT aggregates:
- `SUM(DISTINCT val)` - same pattern, but SUM in stage 2
- `AVG(DISTINCT val)` - same pattern, but AVG in stage 2
- Multiple distinct aggregates would need multiple chains or more complex logic