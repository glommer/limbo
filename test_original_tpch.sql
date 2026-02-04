CREATE TABLE lineitem (
    l_orderkey INTEGER,
    l_partkey INTEGER,
    l_suppkey INTEGER,
    l_linenumber INTEGER,
    l_quantity DECIMAL(15, 2),
    l_extendedprice DECIMAL(15, 2),
    l_discount DECIMAL(15, 2),
    l_tax DECIMAL(15, 2),
    l_returnflag TEXT,
    l_linestatus TEXT,
    l_shipdate DATE,
    l_commitdate DATE,
    l_receiptdate DATE,
    l_shipinstruct TEXT,
    l_shipmode TEXT,
    l_comment TEXT
);

INSERT INTO lineitem VALUES (1, 1, 1, 1, 10.0, 100.0, 0.04, 0.02, 'N', 'O', '1998-01-01', '1998-01-01', '1998-01-01', 'NONE', 'TRUCK', 'test');

CREATE MATERIALIZED VIEW revenue0 AS 
SELECT 
    l_suppkey AS supplier_no,
    sum(l_extendedprice * (1 - l_discount)) AS total_revenue
FROM lineitem
WHERE l_shipdate >= DATE('1996-05-01')
    AND l_shipdate < DATE('1996-05-01', '+3 month')
GROUP BY l_suppkey;
