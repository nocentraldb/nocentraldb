# 2026-11-07 · NOC-6 · A member's log is a table in the local view's SQLite database, appended and folded in one transaction

- Decided by: Carsten (RFC 0002 decision D7)
- Why: appending a change and folding it happen in one SQLite transaction, so after a crash the log and the view always agree, and the storage is the same natively and in the browser.
- Instead of: separate append-only segment files next to the SQLite view.
- Topics: rfc-0002, log, sqlite, on-disk-format

The table's columns are in `docs/rfc/0002-change-and-log-format.md`, "The log on disk".
