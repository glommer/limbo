CREATE TABLE lineitem (
    l_orderkey INTEGER,
    l_partkey INTEGER,
    l_suppkey INTEGER,
    l_linenumber INTEGER,
    l_quantity REAL,
    l_extendedprice REAL,
    l_discount REAL,
    l_tax REAL,
    l_returnflag TEXT,
    l_linestatus TEXT,
    l_shipdate TEXT,
    l_commitdate TEXT,
    l_receiptdate TEXT,
    l_shipinstruct TEXT,
    l_shipmode TEXT,
    l_comment TEXT
);

INSERT INTO lineitem VALUES (1, 1, 1, 1, 10.0, 100.0, 0.04, 0.02, 'N', 'O', '1998-01-01', '1998-01-01', '1998-01-01', 'NONE', 'TRUCK', 'test');
INSERT INTO lineitem VALUES (2, 2, 2, 1, 20.0, 200.0, 0.10, 0.05, 'N', 'O', '1998-06-01', '1998-06-01', '1998-06-01', 'NONE', 'SHIP', 'test');
INSERT INTO lineitem VALUES (3, 3, 3, 1, 15.0, 150.0, 0.05, 0.03, 'R', 'F', '1999-01-01', '1999-01-01', '1999-01-01', 'NONE', 'AIR', 'test');

CREATE MATERIALIZED VIEW tpch_1 AS 
SELECT
    l_returnflag,
    l_linestatus,
    sum(l_quantity) as sum_qty,
    sum(l_extendedprice) as sum_base_price,
    sum(l_extendedprice * (1 - l_discount)) as sum_disc_price,
    sum(l_extendedprice * (1 - l_discount) * (1 + l_tax)) as sum_charge,
    avg(l_quantity) as avg_qty,
    avg(l_extendedprice) as avg_price,
    avg(l_discount) as avg_disc,
    count(*) as count_order
FROM lineitem
WHERE l_shipdate <= CAST('1998-12-01' AS DATETIME)
GROUP BY l_returnflag, l_linestatus;

SELECT * FROM tpch_1;
