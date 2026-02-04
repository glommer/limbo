CREATE TABLE lineitem (
    l_quantity REAL,
    l_returnflag TEXT,
    l_shipdate TEXT
);
INSERT INTO lineitem VALUES (10.0, 'N', '1998-01-01'), (20.0, 'N', '1998-06-01'), (15.0, 'R', '1999-01-01');

CREATE MATERIALIZED VIEW v AS 
SELECT l_returnflag, sum(l_quantity) as total
FROM lineitem
WHERE l_shipdate <= '1998-12-01'
GROUP BY l_returnflag;

SELECT * FROM v;
