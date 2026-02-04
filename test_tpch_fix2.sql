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

-- Insert some test data
INSERT INTO lineitem VALUES (1, 1, 1, 1, 10.0, 100.0, 0.04, 0.02, 'N', 'O', '1998-01-01', '1998-01-01', '1998-01-01', 'NONE', 'TRUCK', 'test');
INSERT INTO lineitem VALUES (2, 2, 2, 1, 20.0, 200.0, 0.10, 0.05, 'N', 'O', '1998-01-02', '1998-01-02', '1998-01-02', 'NONE', 'SHIP', 'test');

CREATE MATERIALIZED VIEW v AS
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
WHERE l_shipdate <= date('1998-12-01')
GROUP BY l_returnflag, l_linestatus;

SELECT * FROM v;
