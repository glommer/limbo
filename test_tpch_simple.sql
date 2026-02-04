CREATE TABLE lineitem (
    l_suppkey INTEGER,
    l_extendedprice REAL,
    l_discount REAL,
    l_shipdate TEXT
);

INSERT INTO lineitem VALUES (1, 100.0, 0.04, '1996-05-15');
INSERT INTO lineitem VALUES (1, 200.0, 0.10, '1996-06-01');
INSERT INTO lineitem VALUES (2, 150.0, 0.05, '1996-05-20');

-- Without function calls in WHERE
CREATE MATERIALIZED VIEW revenue0 AS 
SELECT 
    l_suppkey AS supplier_no,
    sum(l_extendedprice * (1 - l_discount)) AS total_revenue
FROM lineitem
WHERE l_shipdate >= '1996-05-01'
    AND l_shipdate < '1996-08-01'
GROUP BY l_suppkey;

SELECT * FROM revenue0;
