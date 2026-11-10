---
type: internal
tickets: [NOC-174]
components: [core]
---
`ncdb-core`'s view builds and runs on wasm32: rusqlite on `sqlite-wasm-rs` with the `opfs-sahpool` VFS in a dedicated worker, SQLite's in-memory VFS where OPFS is missing, and SQLite 3.54.1 on both targets; a Playwright test runs it in Chromium, Firefox and WebKit.
