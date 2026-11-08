# 2026-11-08 · NOC-12 · RFC 0006: every recommendation accepted

- Decided by: Carsten
- Why: one database per member; the view's layout is not a stable format, only the log is; rusqlite everywhere, with opfs-sahpool in a dedicated worker in the browser; one tab owns the database; UTF-8 byte order in indexes; device disk encryption instead of an encrypted database for v1; synchronous = FULL; persistent storage requested by the SDK; secure_delete on.
- Instead of: the alternatives listed for each decision in `docs/rfc/0006-local-view.md`.
- Topics: rfc-0006, local-view, sqlite
