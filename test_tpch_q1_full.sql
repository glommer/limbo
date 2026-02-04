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

-- Insert test data with dates before and after 1998-12-01
INSERT INTO lineitem VALUES (1, 1, 1, 1, 10.0, 1000.0, 0.05, 0.02, 'N', 'O', '1998-06-01', '1998-07-01', '1998-07-15', 'DELIVER IN PERSON', 'TRUCK', 'test1');
INSERT INTO lineitem VALUES (2, 2, 2, 1, 20.0, 2000.0, 0.10, 0.03, 'N', 'O', '1998-09-01', '1998-10-01', '1998-10-15', 'DELIVER IN PERSON', 'MAIL', 'test2');
INSERT INTO lineitem VALUES (3, 3, 3, 1, 15.0, 1500.0, 0.07, 0.04, 'R', 'F', '1998-11-01', '1998-12-01', '1998-12-15', 'TAKE BACK RETURN', 'SHIP', 'test3');
-- This one should be excluded (after 1998-12-01)
INSERT INTO lineitem VALUES (4, 4, 4, 1, 25.0, 2500.0, 0.08, 0.05, 'A', 'F', '1999-01-01', '1999-02-01', '1999-02-15', 'NONE', 'AIR', 'test4');

CREATE MATERIALIZED VIEW revenue AS
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
FROM
    lineitem
WHERE
    l_shipdate <= CAST('1998-12-01' AS DATETIME)
GROUP BY
    l_returnflag,
    l_linestatus;

SELECT * FROM revenue;