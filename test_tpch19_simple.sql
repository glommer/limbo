CREATE TABLE lineitem (
    l_partkey INTEGER,
    l_quantity REAL,
    l_extendedprice REAL,
    l_discount REAL,
    l_shipmode TEXT,
    l_shipinstruct TEXT
);

CREATE TABLE part (
    p_partkey INTEGER PRIMARY KEY,
    p_brand TEXT,
    p_container TEXT,
    p_size INTEGER
);

-- Insert test data
INSERT INTO part VALUES (1, 'Brand#22', 'SM CASE', 3);
INSERT INTO lineitem VALUES (1, 10.0, 100.0, 0.05, 'AIR', 'DELIVER IN PERSON');

-- Simplified version with BETWEEN and IN
CREATE MATERIALIZED VIEW tpch19_simple AS
SELECT sum(l_extendedprice * (1 - l_discount)) as revenue
FROM lineitem
JOIN part ON l_partkey = p_partkey
WHERE p_size BETWEEN 1 AND 5
  AND l_shipmode IN ('AIR', 'AIR REG');

SELECT * FROM tpch19_simple;
