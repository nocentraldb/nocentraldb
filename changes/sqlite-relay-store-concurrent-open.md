---
type: fix
tickets: [NOC-137]
introduced: cycle
components: [relay]
---
## Symptom
Opening the relay's SQLite store could fail with "journal mode: database is locked".

## When
Several handles opened a new database at the same time.

## Cause
Switching the database to WAL can return SQLITE_BUSY without waiting for the busy timeout.

## Fix
The store retries the switch to WAL until its busy timeout, as it already waits for every write.
