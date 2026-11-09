# 2026-11-08 · NOC-33 · The probe uses SQLite's own opfs-sahpool build, not the Rust spike

- Decided by: coding agent (NOC-33)
- Why: the question is what the browser does with OPFS files, not which binding writes them. `@sqlite.org/sqlite-wasm` (pinned, dev-dependency) ships the `opfs-sahpool` VFS that RFC 0006 picks through its Rust port, needs no Rust build in the probe, and its static files can be copied to any HTTPS host for the manual checks on phones.
- Instead of: building RFC 0006's spike crate (`rusqlite` with `sqlite-wasm-vfs`) to WebAssembly for the probe.
- Topics: browser-storage, sqlite, dependencies
