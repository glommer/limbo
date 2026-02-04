# DBSP Implementation Roadmap for Limbo

## Current State (December 2024)

### What We Have Implemented

1. **Basic Z-set Foundation**
   - `SimpleZSet<T>` with insert/merge operations and weight tracking
   - `SimpleStream<T>` for maintaining current state
   - Positive weights for insertions, negative for deletions

2. **Transaction Isolation for Views**
   - Per-connection `ViewTransactionState` in `Connection` struct
   - Uncommitted changes stored separately from committed view data
   - Proper COMMIT (merge to global) and ROLLBACK (discard) semantics

3. **Simple Incremental Views**
   - Support for `CREATE VIEW ... AS SELECT ... WHERE ...` with basic predicates
   - Incremental updates for INSERT/DELETE on single tables
   - Views automatically update when base tables change

### Limitations of Current Implementation

1. **No Join Support**: Views with JOINs would need full recomputation
2. **No Aggregation Support**: COUNT, SUM, AVG, etc. require full recomputation
3. **No Operator Composition**: Direct table→view connections instead of dataflow graph
4. **Manual Incrementalization**: We handle changes imperatively rather than algebraically
5. **No Query Decomposition**: Complex queries aren't broken into incremental operators

## The Gap: Current vs Full DBSP

### Example: Email Event Counting

Consider this query that we cannot currently incrementalize:
```sql
CREATE TABLE users (user_id INTEGER PRIMARY KEY, email TEXT);
CREATE TABLE events (event_id INTEGER PRIMARY KEY, user_id INTEGER, event_type TEXT, timestamp INTEGER);
CREATE VIEW email_event_counts AS 
  SELECT u.email, COUNT(*) as event_count
  FROM users u 
  JOIN events e ON u.user_id = e.user_id
  GROUP BY u.email;
```

**Full DBSP would**: 
- Incrementally update only affected email counts when events are added
- Use the formula: `∂(JOIN) = left ⋈ ∂right + ∂left ⋈ right + ∂left ⋈ ∂right`

**Our current system**: 
- Cannot handle this query incrementally
- Would need to recompute entire join and aggregation

## Implementation Roadmap

### Phase 1: Query Decomposition Framework ✅ Current Focus
**Goal**: Build infrastructure to represent queries as operator DAGs

**Test Query**:
```sql
CREATE VIEW simple_filtered AS 
  SELECT id, name FROM items WHERE status = 'active';
```

**SQL Verification** (tests correctness only):
```sql
INSERT INTO items VALUES (1, 'A', 'active'), (2, 'B', 'inactive');
SELECT * FROM simple_filtered; -- Should show: 1|A
INSERT INTO items VALUES (3, 'C', 'active');
SELECT * FROM simple_filtered; -- Should show: 1|A and 3|C
```

**Unit Test Verification** (tests incrementality):
```rust
// In core/incremental/tests.rs or similar
#[test]
fn test_query_decomposition() {
    // Parse the view query
    let query = "SELECT id, name FROM items WHERE status = 'active'";
    let dag = decompose_query(query);
    
    // Verify correct operator structure
    assert_eq!(dag.operators.len(), 2);
    assert!(matches!(dag.operators[0], QueryOperator::Filter { .. }));
    assert!(matches!(dag.operators[1], QueryOperator::Project { .. }));
    
    // Verify operators are connected correctly
    assert_eq!(dag.operators[1].inputs, vec![0]); // Project takes Filter's output
}

#[test]
fn test_incremental_filter_operator() {
    let mut filter = FilterOperator::new("status = 'active'");
    
    // Initial data
    let initial = vec![
        (1, vec![Value::Integer(1), Value::Text("A"), Value::Text("active")]),
        (2, vec![Value::Integer(2), Value::Text("B"), Value::Text("inactive")]),
    ];
    filter.initialize(initial);
    
    // Add one row - should only process this row, not refilter everything
    let delta = ZSet::singleton(3, vec![Value::Integer(3), Value::Text("C"), Value::Text("active")], 1);
    let output_delta = filter.process_delta(delta);
    
    // Verify we only output the new matching row
    assert_eq!(output_delta.len(), 1);
    assert_eq!(output_delta.get(&3), Some(&1)); // Row 3 with weight +1
}

#[test] 
fn test_no_recomputation_on_insert() {
    let mut view = IncrementalView::from_sql("SELECT * FROM items WHERE status = 'active'");
    
    // Track computation calls
    let mut computation_counter = ComputationTracker::new();
    view.set_tracker(&computation_counter);
    
    // Initial population
    view.populate(initial_data);
    let initial_computations = computation_counter.count();
    
    // Insert one row
    view.process_insert(new_row);
    let insert_computations = computation_counter.count() - initial_computations;
    
    // Should only process the new row, not recompute entire view
    assert_eq!(insert_computations, 1);
    assert!(insert_computations < initial_data.len());
}
```

**TODO**:
- [ ] Create `QueryOperator` enum (Select, Project, Filter, Join, Aggregate)
- [ ] Build query plan representation with operator DAG
- [ ] Implement operator DAG construction from SQL
- [ ] Add decomposition unit tests in `core/incremental/tests.rs`
- [ ] Add computation tracking to verify incremental behavior
- [ ] Ensure operators process only deltas, not full datasets

### Phase 2: Incremental Join Operator
**Goal**: Implement incremental join using DBSP algebra

**Test Query**:
```sql
CREATE TABLE emails (user_id INTEGER PRIMARY KEY, email TEXT);
CREATE TABLE logins (login_id INTEGER PRIMARY KEY, user_id INTEGER, timestamp INTEGER);
CREATE VIEW user_logins AS 
  SELECT e.email, l.timestamp 
  FROM emails e 
  JOIN logins l ON e.user_id = l.user_id;
```

**SQL Verification** (correctness):
```sql
INSERT INTO emails VALUES (1, 'alice@example.com'), (2, 'bob@example.com');
INSERT INTO logins VALUES (1, 1, 1000), (2, 1, 2000);
SELECT * FROM user_logins ORDER BY timestamp; 
-- Should show: alice@example.com|1000, alice@example.com|2000

INSERT INTO logins VALUES (3, 2, 3000);
SELECT * FROM user_logins ORDER BY timestamp; 
-- Should incrementally add: bob@example.com|3000
```

**Unit Test Verification** (incrementality):
```rust
#[test]
fn test_incremental_join_formula() {
    let mut join = IncrementalJoin::new("user_id");
    
    // Initial data
    let emails = ZSet::from(vec![(1, "alice@example.com"), (2, "bob@example.com")]);
    let logins = ZSet::from(vec![(1, 1000), (1, 2000)]);
    join.initialize(emails.clone(), logins.clone());
    
    // Add one login - should use incremental formula
    let delta_logins = ZSet::singleton((2, 3000), 1);
    
    // Track that we use the formula: ∂(A ⋈ B) = A ⋈ ∂B + ∂A ⋈ B + ∂A ⋈ ∂B
    let output = join.process_delta(ZSet::empty(), delta_logins);
    
    // Should compute: emails ⋈ delta_logins only (since delta_emails is empty)
    assert_eq!(output.len(), 1); // Only bob@example.com|3000
    
    // Verify we didn't recompute alice's joins
    assert_eq!(join.computation_count(), 1); // Only processed new join
}

#[test]
fn test_join_uses_index_not_nested_loop() {
    let mut join = IncrementalJoin::new("user_id");
    
    // Build index on emails
    join.build_index(EmailsTable, "user_id");
    
    // Insert login - should use index lookup, not scan all emails
    let lookup_count = join.process_with_tracking(new_login);
    
    assert_eq!(lookup_count, 1); // Single index lookup
    assert_ne!(lookup_count, total_email_count); // Not scanning all emails
}
```

**TODO**:
- [ ] Implement `IncrementalJoin` operator
- [ ] Add join delta computation: `∂(A ⋈ B) = A ⋈ ∂B + ∂A ⋈ B + ∂A ⋈ ∂B`
- [ ] Maintain join indexes for efficient lookups
- [ ] Add unit tests verifying incremental formula is used
- [ ] Add performance counters to track computation count
- [ ] Verify index-based lookups, not nested loops

### Phase 3: Incremental Aggregation
**Goal**: Support COUNT, SUM, AVG incrementally

**Test Query**:
```sql
CREATE TABLE orders (order_id INTEGER PRIMARY KEY, user_id INTEGER, amount INTEGER);
CREATE VIEW user_totals AS 
  SELECT user_id, COUNT(*) as order_count, SUM(amount) as total_amount 
  FROM orders 
  GROUP BY user_id;
```

**Verification Test**:
```sql
INSERT INTO orders VALUES (1, 1, 100), (2, 1, 200), (3, 2, 150);
SELECT * FROM user_totals ORDER BY user_id;
-- Should show: 1|2|300, 2|1|150

INSERT INTO orders VALUES (4, 1, 50);
SELECT * FROM user_totals ORDER BY user_id;
-- Should incrementally update: 1|3|350, 2|1|150
-- COUNT incremented by 1, SUM incremented by 50
```

**TODO**:
- [ ] Implement `IncrementalAggregate` operator
- [ ] Support COUNT (add/subtract counts)
- [ ] Support SUM (add/subtract values)
- [ ] Support AVG (maintain sum and count separately)
- [ ] Handle GROUP BY with incremental grouping

### Phase 4: Complex Query Composition
**Goal**: Combine joins and aggregations

**Test Query** (Email Event Counting):
```sql
CREATE TABLE users (user_id INTEGER PRIMARY KEY, email TEXT);
CREATE TABLE events (event_id INTEGER PRIMARY KEY, user_id INTEGER, event_type TEXT);
CREATE VIEW email_event_counts AS 
  SELECT u.email, COUNT(*) as event_count
  FROM users u 
  JOIN events e ON u.user_id = e.user_id
  GROUP BY u.email;
```

**Verification Test**:
```sql
INSERT INTO users VALUES (1, 'alice@example.com'), (2, 'bob@example.com');
INSERT INTO events VALUES (1, 1, 'login'), (2, 1, 'purchase');
SELECT * FROM email_event_counts ORDER BY email;
-- Should show: alice@example.com|2, bob@example.com|0

INSERT INTO events VALUES (3, 2, 'login');
SELECT * FROM email_event_counts ORDER BY email;
-- Should incrementally update: alice@example.com|2, bob@example.com|1
```

**TODO**:
- [ ] Implement operator chaining
- [ ] Ensure deltas flow through operator DAG
- [ ] Optimize redundant computations
- [ ] Add complex composition tests

### Phase 5: Advanced DBSP Features
**Goal**: Full DBSP algebra implementation

**Features**:
- [ ] Recursive queries (fixed-point computation)
- [ ] Window functions with incremental windows
- [ ] DISTINCT with incremental deduplication
- [ ] Set operations (UNION, INTERSECT, EXCEPT)
- [ ] Subqueries and correlated subqueries
- [ ] Automatic differentiation of arbitrary queries

## Test Cases for `testing/views.test`

### Basic Join Test (Phase 2)
```tcl
do_execsql_test_on_specific_db :memory: view_incremental_join {
    CREATE TABLE users (user_id INTEGER PRIMARY KEY, email TEXT);
    CREATE TABLE events (event_id INTEGER PRIMARY KEY, user_id INTEGER, action TEXT);
    INSERT INTO users VALUES (1, 'alice@example.com'), (2, 'bob@example.com');
    INSERT INTO events VALUES (1, 1, 'login'), (2, 1, 'logout');
    
    CREATE VIEW user_events AS 
      SELECT u.email, e.action 
      FROM users u 
      JOIN events e ON u.user_id = e.user_id;
    
    SELECT * FROM user_events ORDER BY email, action;
} {alice@example.com|login
alice@example.com|logout}

do_execsql_test_on_specific_db :memory: view_incremental_join_update {
    INSERT INTO events VALUES (3, 2, 'purchase');
    SELECT * FROM user_events ORDER BY email, action;
} {alice@example.com|login
alice@example.com|logout
bob@example.com|purchase}
```

### Aggregation Test (Phase 3)
```tcl
do_execsql_test_on_specific_db :memory: view_incremental_aggregation {
    CREATE TABLE sales (sale_id INTEGER PRIMARY KEY, product TEXT, amount INTEGER);
    INSERT INTO sales VALUES (1, 'Widget', 100), (2, 'Gadget', 200), (3, 'Widget', 150);
    
    CREATE VIEW product_totals AS 
      SELECT product, COUNT(*) as count, SUM(amount) as total 
      FROM sales 
      GROUP BY product;
    
    SELECT * FROM product_totals ORDER BY product;
} {Gadget|1|200
Widget|2|250}

do_execsql_test_on_specific_db :memory: view_incremental_aggregation_update {
    INSERT INTO sales VALUES (4, 'Widget', 50);
    SELECT * FROM product_totals ORDER BY product;
} {Gadget|1|200
Widget|3|300}
```

### Complex Join+Aggregation Test (Phase 4)
```tcl
do_execsql_test_on_specific_db :memory: view_join_aggregation {
    CREATE TABLE customers (customer_id INTEGER PRIMARY KEY, name TEXT, country TEXT);
    CREATE TABLE orders (order_id INTEGER PRIMARY KEY, customer_id INTEGER, amount INTEGER);
    
    INSERT INTO customers VALUES (1, 'Alice', 'USA'), (2, 'Bob', 'UK'), (3, 'Charlie', 'USA');
    INSERT INTO orders VALUES (1, 1, 100), (2, 1, 200), (3, 2, 150), (4, 3, 300);
    
    CREATE VIEW country_sales AS 
      SELECT c.country, COUNT(*) as order_count, SUM(o.amount) as total_sales
      FROM customers c 
      JOIN orders o ON c.customer_id = o.customer_id
      GROUP BY c.country;
    
    SELECT * FROM country_sales ORDER BY country;
} {UK|1|150
USA|3|600}

do_execsql_test_on_specific_db :memory: view_join_aggregation_incremental {
    INSERT INTO orders VALUES (5, 2, 250);
    SELECT * FROM country_sales ORDER BY country;
} {UK|2|400
USA|3|600}
```

## Success Metrics

1. **Correctness**: All test cases pass with same results as SQLite
2. **Performance**: Incremental updates are O(change_size) not O(data_size)
3. **Memory**: Maintain reasonable memory usage for materialized state
4. **Completeness**: Support full SQL query language incrementally

## Implementation Strategy

### Using Feldera DBSP as Reference

We have access to two key Feldera components:
1. **DBSP crate** at `/Users/glaubercosta/feldera/crates/dbsp` - The runtime implementation
2. **SQL-to-DBSP compiler** at `/Users/glaubercosta/feldera/sql-to-dbsp-compiler` - Translates SQL to DBSP circuits

**IMPORTANT: Implementation Approach (in order of preference):**

1. **Import verbatim or quasi-verbatim**: When possible, import DBSP files directly, keeping their MIT license header and attributing the Feldera crate. This is preferred for core algorithms and data structures that don't introduce new dependencies.

2. **Study and adapt**: Read their code to understand the implementation before writing ours. Avoid verbatim copying when their code uses dependencies we don't have (e.g., Tokio, Calcite, which are not acceptable for Turso).

3. **Write new code**: Only if options 1 and 2 fail, write completely new implementations.

### Key Feldera Components to Reference

#### DBSP Runtime (`/crates/dbsp/`)

**Core Algebra** (`src/algebra/`):
- `zset/mod.rs` - Z-set implementation with weights
- `lattice.rs` - Lattice operations for timestamps
- `order.rs` - Ordering traits

**Operators** (`src/operator/`):
- `join.rs` - Incremental join implementation
- `aggregate.rs` - Aggregation operators
- `distinct.rs` - Distinct operator
- `filter_map.rs` - Filter and map operators
- `group/` - Group-by operations
- `dynamic/` - Dynamic dispatch versions

**Trace** (`src/trace/`):
- Core batch and trace implementations
- `ord/` - Ordered collections (likely what we need)
- `layers/` - Layered trace structure

#### SQL-to-DBSP Compiler (`/sql-to-dbsp-compiler/`)

**Circuit Operators** (`SQL-compiler/src/main/java/org/dbsp/sqlCompiler/circuit/operator/`):
- `DBSPFilterOperator.java` - Filter implementation
- `DBSPJoinOperator.java` - Join operator
- `DBSPAggregateLinearPostprocessOperator.java` - Aggregations
- `DBSPMapIndexOperator.java` - Projection/mapping
- `DBSPDistinctOperator.java` - Distinct values

**Simplified Simulator** (`simulator/src/main/java/org/dbsp/simulator/`):
- `operators/FilterOperator.java` - Simple filter implementation
- `operators/JoinOperator.java` - Join logic
- `collections/ZSet.java` - Z-set collection
- Shows clean separation of operators from runtime complexity

**Key Insights from SQL Compiler**:
1. They use Calcite for SQL parsing and optimization (we use our own parser)
2. Operators are chained to form a circuit/DAG
3. Each operator has clear input/output ports
4. The simulator shows a simpler implementation without Calcite dependencies

### Dependencies to Avoid

The Feldera implementation uses several dependencies we cannot adopt:
- **Tokio**: Their async runtime - Turso uses its own async design with state machines
- **Calcite**: SQL parser and optimizer - we use our own parser
- **rkyv**: Serialization - we have our own approach
- Any networking/communication layers

### Architecture Adaptation

Turso follows an async design using state machines (not Tokio). Our DBSP implementation should:
- Use state machines for async operations, consistent with existing Turso code
- Integrate with Turso's existing async I/O patterns
- Avoid external async runtimes while maintaining async capabilities

### What We Can Import Directly

Based on initial inspection, these components may be importable with minimal changes:
- Basic Z-set algebra and operations
- Core operator algorithms (join formulas, aggregation logic)
- Batch and trace data structures (without persistence layer)
- Mathematical foundations (lattices, ordering)

## References

- [DBSP Paper (VLDB 2023)](https://www.vldb.org/pvldb/vol16/p1601-budiu.pdf)
- [Feldera/DBSP Rust Implementation](https://github.com/feldera/feldera) - Local copy at `/Users/glaubercosta/feldera/crates/dbsp`
- [Differential Dataflow](https://github.com/TimelyDataflow/differential-dataflow)

## Notes for Future Sessions

When continuing work, refer to:
1. This roadmap for current phase and next steps
2. Test cases to verify implementation
3. Current limitations to understand what needs building
4. The theoretical gap between our implementation and full DBSP

Current working directory: `/Users/glaubercosta/limbo`
Current branch: `findbug`
Test command: `./turso-test.sh` or `./testing/views.test`