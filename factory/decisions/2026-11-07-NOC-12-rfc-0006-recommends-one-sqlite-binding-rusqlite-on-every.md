# 2026-11-07 · NOC-12 · RFC 0006 recommends one SQLite binding (rusqlite) on every target

- Decided by: coding agent (NOC-12), for the owner to confirm in the RFC's decision 3
- Why: rusqlite drives sqlite-wasm-rs in the browser, so the fold is one body of Rust code (principle 7); the spike shows it works with OPFS in a worker.
- Instead of: the official SQLite wasm build or wa-sqlite, both driven through JavaScript, which would fork the binding by platform.
- Topics: rfc-0006, local-view, sqlite
