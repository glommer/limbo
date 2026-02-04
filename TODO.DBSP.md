  SELECT c.*, o.total
  FROM customers c
  JOIN orders o ON c.id = o.customer_id
  WHERE c.id > 5 AND o.total > 100
  UNION ALL
  SELECT c.*, o.total
  FROM customers c
  JOIN orders o ON c.id = o.customer_id
  WHERE c.id < 3 AND o.total < 50

  The error occurs when the incremental compiler tries to process the JOIN condition c.id = o.customer_id. It incorrectly looks for the column customer_id
  in table c (customers) instead of table o (orders), resulting in:

  Error: Column 'customer_id' with table Some("c") not found in schema


  CREATE MATERIALIZED VIEW mv3 AS SELECT * FROM t4 JOIN t5 USING (a)

  Issue: The materialized view returns incorrect values for column c:
  - Expected (from regular SELECT): 100, 200
  - Actual (from materialized view): 1, 2

* We need to store the column name -> index mapping in the state of the circuit too.
* make sure that all the hashes we use are sane
* add integrity check for the materialized views
* implement fuzzer
* consider emitting custom VDBE for the circuit.
* test with very large databases to test I/O path
* in-memory circuit for easy testing (?)
* create materialized view now calls parseschema, but we could do better, like Levy did
* column names are wrong in the view
* benchmark
