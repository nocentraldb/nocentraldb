# 2026-11-08 · NOC-12 · RFC 0006: the owner decides as recommended, one database per member with rusqlite on every target

- Decided by: Carsten
- Why: one database per member; the view's layout is not a stable format, only the log is; rusqlite everywhere, driving sqlite-wasm-rs with opfs-sahpool in a dedicated worker in the browser, so the fold is one body of Rust code (principle 7); one tab owns the database; UTF-8 byte order in indexes; device disk encryption instead of an encrypted database for v1; synchronous = FULL; persistent storage requested by the SDK; secure_delete on.
- Instead of: the alternatives listed for each decision in `docs/rfc/0006-local-view.md`, among them the official SQLite wasm build or wa-sqlite driven through JavaScript, which would fork the binding by platform.
- Topics: rfc-0006, local-view, sqlite
