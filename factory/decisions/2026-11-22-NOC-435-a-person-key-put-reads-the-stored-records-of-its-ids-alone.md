# 2026-11-22 · NOC-435 · A person-key put reads the stored records of its ids alone, through the store's person_keys_of

- Decided by: coding agent (NOC-435)
- Why: a put read every person-key record of the space to learn the generations its records replace, so its cost grew with the persons in the space. `RelayStore::person_keys_of` reads only the records of the given ids, and every backend implements it as a lookup per id (SQLite: one query on the primary key per id; DynamoDB: one consistent `GetItem` per id, the operation the backend already uses and the AWS Emulator covers). A put touches few ids, and DynamoDB caps it at 100, so a put costs reads by its records, not by the space. What the relay stores does not change.
- Instead of: one DynamoDB `BatchGetItem` per put (fewer round trips for a put of many records, but an operation the backend does not use yet and the emulator coverage does not check), or keeping the full read of the space's records
- Topics: relay, performance, aws
