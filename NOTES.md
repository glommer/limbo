* view creation failure is leaving views behind - we had fixed it already, but likely part of the code we lost.
* columns not shown correct for "select a + 10 from t" (table)
* if an mview is not pure aggregates, error out. Postgres does it.
* if a function is an aggregate but not supported by the operator, error out.
