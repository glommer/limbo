# DataFusion Integration with Limbo

## Summary

We've successfully created a proof of concept for integrating DataFusion's LogicalPlan concept with Limbo. The main challenge was a dependency conflict between Arrow (required by DataFusion) and Limbo's chrono version.

## The Problem

1. **Arrow Dependency Conflict**: DataFusion depends on Arrow, which has a conflict with chrono 0.4.41
   - Arrow defines `ChronoDateExt::quarter()` trait method
   - Chrono 0.4.41 also defines `Datelike::quarter()` trait method
   - This creates ambiguity in arrow-arith that prevents compilation

2. **Why Not Downgrade Chrono?**: Limbo uses chrono 0.4.38+ which already has the `quarter()` method

## Solutions

### Solution 1: Use sqlparser Directly (Implemented)
We can parse SQL using sqlparser with SQLite dialect and create our own LogicalPlan structure:

```rust
// See datafusion_concept_demo.rs
let dialect = SQLiteDialect {};
let ast = Parser::parse_sql(&dialect, sql)?;
// Convert AST to our LogicalPlan structure
```

This works perfectly for your complex query:
```sql
SELECT COUNT(a) FROM (SELECT HEX(SUM(a * 2)) AS a FROM t WHERE b > 2 GROUP BY b)
```

### Solution 2: Fork and Fix Arrow (Recommended for Production)
1. Fork arrow-rs
2. Fix the ambiguity in arrow-arith/src/temporal.rs:91
   ```rust
   // Change from:
   DatePart::Quarter => |d| d.quarter() as i32,
   // To:
   DatePart::Quarter => |d| ChronoDateExt::quarter(&d) as i32,
   ```
3. Use your fork in Cargo.toml:
   ```toml
   [patch.crates-io]
   arrow = { git = "https://github.com/yourusername/arrow-rs", branch = "fix-chrono-conflict" }
   ```

### Solution 3: Use DataFusion Without Arrow Features
Unfortunately, even with `default-features = false`, datafusion-common still pulls in arrow. This approach doesn't work.

### Solution 4: Wait for Upstream Fix
Submit a PR to arrow-rs to fix the ambiguity. The issue affects anyone using arrow with chrono 0.4.41+.

## What We Built

### 1. Concept Demo (`datafusion_concept_demo.rs`)
- Parses SQL with SQLite dialect
- Converts to a LogicalPlan-like structure
- Successfully handles:
  - Nested subqueries
  - Aggregations (COUNT, SUM)
  - GROUP BY and HAVING
  - JOINs and UNIONs
  - Window functions
  - CTEs

### 2. Minimal Parser (`datafusion_minimal.rs`)
- Shows how to use just sqlparser without DataFusion
- Demonstrates parsing complex SQL queries

### 3. Full Integration Template (`datafusion_integration.rs`)
- Shows how the full integration would work
- Currently commented out due to arrow conflict
- Can be enabled once arrow is fixed

## Next Steps for DBSP Compiler

1. **Fix Arrow Dependency**:
   - Fork arrow-rs and apply the fix
   - Or submit PR upstream

2. **Complete DataFusion Integration**:
   - Enable datafusion_integration.rs
   - Implement ContextProvider for Limbo's schema
   - Map Limbo tables to DataFusion TableSource

3. **Add DBSP Layer**:
   - Convert LogicalPlan to DBSP operators
   - Implement incrementalization (Q^Δ = D ∘ Q ∘ I)
   - Add integrate/differentiate operators

4. **Generate Rust Code**:
   - Convert DBSP operators to Rust code
   - Or interpret directly at runtime

## Testing

All tests pass with the concept demo:
```bash
cargo test -p turso_core datafusion_concept_demo::tests
```

The complex nested query is successfully parsed and converted to a logical plan structure.