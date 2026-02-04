why use an atomic id instead of hashing the database name?
in wal type we use database_id being 0 - 4, so maybe leave space for that instead of allowing just two booleans?
maybe database _id has to be in a shared file ?
