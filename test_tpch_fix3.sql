CREATE TABLE lineitem (
    l_quantity REAL,
    l_extendedprice REAL,
    l_discount REAL,
    l_tax REAL
);

-- Insert test data
INSERT INTO lineitem VALUES (10.0, 100.0, 0.04, 0.02);
INSERT INTO lineitem VALUES (20.0, 200.0, 0.10, 0.05);

-- Test the expression that was failing
CREATE MATERIALIZED VIEW v AS
SELECT
    sum(l_extendedprice * (1 - l_discount)) as sum_disc_price,
    sum(l_extendedprice * (1 - l_discount) * (1 + l_tax)) as sum_charge
FROM lineitem;

SELECT * FROM v;

-- Verify result manually
SELECT 
    l_extendedprice,
    l_discount,
    1 - l_discount as one_minus_disc,
    l_extendedprice * (1 - l_discount) as disc_price
FROM lineitem;
