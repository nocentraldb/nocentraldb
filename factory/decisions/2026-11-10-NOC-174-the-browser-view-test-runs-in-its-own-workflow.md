# 2026-11-10 · NOC-174 · The browser view test runs in its own workflow, outside `pnpm gates`; the view's Rust tests run on wasm32 inside it

- Decided by: coding agent (NOC-174)
- Why: NOC-33 kept browsers out of `pnpm gates` (browser-storage.yml), and the same reasons hold. `browser-view.yml` runs `repo:browser-view` in Chromium, Firefox and WebKit on pull requests that touch the core, and the test names the failing engine. The gate still covers the view on wasm32: its tests no longer carry a native-only gate and run in Node on SQLite's in-memory VFS, except WAL mode and the crash inside a fold (`view_storage_native.rs`), which hold natively only. NOC-178 decides whether a browser joins the gate.
- Instead of: adding the three browsers to `ci.yml` and `pnpm gates`, or running the browser test only by hand.
- Topics: browser, ci, gates, testing, wasm
