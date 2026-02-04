CREATE TABLE lineitem (
    l_partkey INTEGER,
    l_quantity REAL,
    l_extendedprice REAL,
    l_discount REAL,
    l_shipmode TEXT,
    l_shipinstruct TEXT
);

CREATE TABLE part (
    p_partkey INTEGER,
    p_brand TEXT,
    p_container TEXT,
    p_size INTEGER
);

INSERT INTO part VALUES (1, 'Brand#22', 'SM CASE', 3);
INSERT INTO part VALUES (2, 'Brand#23', 'MED BOX', 7);
INSERT INTO part VALUES (3, 'Brand#12', 'LG PACK', 12);
INSERT INTO part VALUES (4, 'Brand#99', 'XL BOX', 20);

INSERT INTO lineitem VALUES (1, 10.0, 1000.0, 0.05, 'AIR', 'DELIVER IN PERSON');
INSERT INTO lineitem VALUES (2, 15.0, 1500.0, 0.10, 'AIR REG', 'DELIVER IN PERSON');
INSERT INTO lineitem VALUES (3, 28.0, 2800.0, 0.07, 'AIR', 'DELIVER IN PERSON');
INSERT INTO lineitem VALUES (4, 50.0, 5000.0, 0.02, 'TRUCK', 'NONE');

CREATE MATERIALIZED VIEW tpch19 AS SELECT
    sum(l_extendedprice * (1 - l_discount)) as revenue
FROM lineitem
JOIN part ON l_partkey = p_partkey
WHERE
    (
        p_brand = 'Brand#22'
        AND (p_container = 'SM CASE' OR p_container = 'SM BOX' OR p_container = 'SM PACK' OR p_container = 'SM PKG')
        AND l_quantity >= 8 AND l_quantity <= 8 + 10
        AND p_size BETWEEN 1 AND 5
        AND l_shipmode IN ('AIR', 'AIR REG')
        AND l_shipinstruct = 'DELIVER IN PERSON'
    )
    OR
    (
        p_brand = 'Brand#23'
        AND (p_container = 'MED BAG' OR p_container = 'MED BOX' OR p_container = 'MED PKG' OR p_container = 'MED PACK')
        AND l_quantity >= 10 AND l_quantity <= 10 + 10
        AND p_size BETWEEN 1 AND 10
        AND l_shipmode IN ('AIR', 'AIR REG')
        AND l_shipinstruct = 'DELIVER IN PERSON'
    )
    OR
    (
        p_brand = 'Brand#12'
        AND (p_container = 'LG CASE' OR p_container = 'LG BOX' OR p_container = 'LG PACK' OR p_container = 'LG PKG')
        AND l_quantity >= 24 AND l_quantity <= 24 + 10
        AND p_size BETWEEN 1 AND 15
        AND l_shipmode IN ('AIR', 'AIR REG')
        AND l_shipinstruct = 'DELIVER IN PERSON'
    );

SELECT * FROM tpch19;