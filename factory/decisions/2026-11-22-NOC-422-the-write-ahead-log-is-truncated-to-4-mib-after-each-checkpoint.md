# 2026-11-22 · NOC-422 · The write-ahead log is truncated to 4 MiB after each checkpoint

- Decided by: coding agent (NOC-422)
- Why: SQLite checkpoints the write-ahead log at 1,000 pages, during a catch-up too, but keeps the file at the largest size one transaction grew it to until the database closes: 187 MB next to a 184 MB database after a new device caught up 81,000 records. `PRAGMA journal_size_limit = 4194304` has SQLite truncate it to 4 MiB, its size between two checkpoints, when it starts the log again at the first write after a checkpoint. It is a setting of the connection, not of the file, so the on-disk format does not change.
- Instead of: checkpointing more often during a catch-up, which the autocheckpoint already does, or with `wal_checkpoint(TRUNCATE)` after each large transaction, which would wait on other connections' readers.
- What this does not do: while one transaction runs, the log still grows to the pages it writes; a catch-up commits a pull's changes in one transaction. A browser keeps a rollback journal, where the setting does not apply.
- Topics: core, local-view, storage
