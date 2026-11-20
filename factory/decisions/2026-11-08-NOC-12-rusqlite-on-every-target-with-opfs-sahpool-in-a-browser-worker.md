# 2026-11-08 · NOC-12 · rusqlite drives SQLite on every target, with sqlite-wasm-rs and opfs-sahpool in a browser worker

- Decided by: Carsten (RFC 0006 decision 3)
- Why: one SQLite binding and one body of fold code on every target (principle 7), and the spike showed it works; `rusqlite` is pinned, with `bundled` natively. The risk is two young crates, `sqlite-wasm-rs` and `sqlite-wasm-vfs`; the fallback is to fork them.
- Instead of: the official SQLite WebAssembly build or wa-sqlite in the browser, called through JavaScript, which would fork the binding and behaviour by platform; the system's SQLite natively.
- Topics: rfc-0006, local-view, sqlite, wasm, browser

`docs/rfc/0006-local-view.md`, "SQLite on each target", gives the stack per target and the options not taken.
