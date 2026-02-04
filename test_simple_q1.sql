CREATE TABLE lineitem (l_quantity REAL, l_returnflag TEXT);
INSERT INTO lineitem VALUES (10.0, 'N'), (20.0, 'N');

CREATE MATERIALIZED VIEW v AS 
SELECT l_returnflag, sum(l_quantity) as total
FROM lineitem
GROUP BY l_returnflag;

SELECT * FROM v;
