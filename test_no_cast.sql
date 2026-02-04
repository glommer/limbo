CREATE TABLE lineitem (
    l_shipdate TEXT,
    l_quantity REAL,
    l_extendedprice REAL,
    l_discount REAL
);

INSERT INTO lineitem VALUES ('1998-01-01', 10.0, 100.0, 0.05);
INSERT INTO lineitem VALUES ('1999-01-01', 20.0, 200.0, 0.10);

-- Test without CAST - just direct comparison
CREATE MATERIALIZED VIEW test_simple AS
SELECT 
    sum(l_extendedprice * (1 - l_discount)) as revenue
FROM lineitem
WHERE l_shipdate <= '1998-12-01';

SELECT * FROM test_simple;
