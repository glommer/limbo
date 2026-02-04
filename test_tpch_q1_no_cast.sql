-- Query without CAST - should work in SQLite
CREATE TABLE lineitem (
    l_quantity REAL,
    l_extendedprice REAL,
    l_discount REAL,
    l_tax REAL,
    l_returnflag TEXT,
    l_linestatus TEXT,
    l_shipdate TEXT
);

INSERT INTO lineitem VALUES (10.0, 1000.0, 0.05, 0.02, 'N', 'O', '1998-06-01');
INSERT INTO lineitem VALUES (20.0, 2000.0, 0.10, 0.03, 'N', 'O', '1998-09-01');
INSERT INTO lineitem VALUES (15.0, 1500.0, 0.07, 0.04, 'R', 'F', '1998-11-01');
INSERT INTO lineitem VALUES (25.0, 2500.0, 0.08, 0.05, 'A', 'F', '1999-01-01');

SELECT
    l_returnflag,
    l_linestatus,
    sum(l_quantity) as sum_qty,
    count(*) as count_order
FROM lineitem
WHERE l_shipdate <= '1998-12-01'
GROUP BY l_returnflag, l_linestatus
ORDER BY l_returnflag, l_linestatus;