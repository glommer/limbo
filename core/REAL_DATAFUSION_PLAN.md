# What Real DataFusion Would Generate

For your query:
```sql
SELECT COUNT(a) FROM (SELECT SUM(a * 2) AS a FROM t WHERE b > 2 GROUP BY b)
```

## Actual DataFusion LogicalPlan Structure

```
Projection: COUNT(#a)
  Aggregate: groupBy=[], aggr=[COUNT(#a)]
    SubqueryAlias: subquery
      Projection: #SUM(t.a * 2) AS a
        Aggregate: groupBy=[#t.b], aggr=[SUM(#t.a * Int64(2))]
          Projection: #t.a * Int64(2), #t.b
            Filter: #t.b > Int64(2)
              TableScan: t projection=[a, b]
```

## Key Differences from Our Simplified Version

1. **Explicit Projection for Arithmetic**:
   - DataFusion adds `Projection: #t.a * Int64(2), #t.b` before the aggregate
   - This computes `a * 2` as a new column before aggregation

2. **Type Information**:
   - Constants have types: `Int64(2)` not just `2`
   - Column references are qualified: `#t.a` not just `a`

3. **Aggregate Decomposition**:
   - The outer COUNT gets its own Aggregate node
   - GROUP BY [] for the outer aggregate (no grouping)

4. **Column Projections**:
   - TableScan specifies which columns to read: `projection=[a, b]`
   - Avoids reading column `c` if it exists but isn't needed

## Why This Matters for DBSP

In DBSP, the decomposition is crucial for incrementalization:

```
// Original: SUM(a * 2)
// Becomes: SUM ∘ (λx. x * 2)

// Incremental version:
// ΔSUM(Δ(a * 2)) = SUM(prev(a * 2)) + SUM(curr(a * 2)) - SUM(removed(a * 2))
```

The projection for `a * 2` needs to be:
1. Computed before aggregation
2. Incrementalized separately
3. Cached if used multiple times

## Execution Flow

```
1. TableScan: Read columns a, b from table t
2. Filter: Keep only rows where b > 2
3. Projection: Compute a * 2 for each row → creates temporary column
4. Aggregate: GROUP BY b, SUM(temporary column)
5. Projection: Rename result as 'a'
6. SubqueryAlias: Name it for outer query
7. Aggregate: No GROUP BY, COUNT(a)
8. Projection: Final result column
```

## In Terms of Operators

Real DataFusion would use these operators:

1. **TableScan** → Stream of (a, b) tuples
2. **FilterExec** → Stream of filtered tuples
3. **ProjectionExec** → Stream of (a*2, b) tuples
4. **AggregateExec** → Stream of (b, sum) groups
5. **ProjectionExec** → Rename to (a)
6. **AggregateExec** → Single COUNT result

Your observation is spot-on: the arithmetic expression `a * 2` should be its own projection step, not embedded in the aggregate!