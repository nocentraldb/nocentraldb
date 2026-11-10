# 2026-11-10 · NOC-174 · In the browser, `view::browser::install` makes opfs-sahpool SQLite's default VFS, and `View::open` stays the same on every target

- Decided by: coding agent (NOC-174)
- Why: RFC 0006 has one asynchronous step in the browser, installing the VFS once per worker, and wants every other call the same on both targets. Making `opfs-sahpool` the default VFS lets `View::open(path, lock)` run unchanged in the worker; without OPFS the in-memory VFS stays the default (decision 12). `install` falls back to memory only when OPFS is missing (the API is absent, or the browser refuses the OPFS root, as in a private window of Firefox or WebKit); outside a dedicated worker it refuses (`NotInWorker`), and any other OPFS failure, such as a pool another tab's worker holds, is an error, never a silent copy in memory. The SDK's `storage` report (NOC-175) maps `Storage::Memory` to `temporary`.
- Instead of: a browser-only `View::open_opfs(name)` that names the VFS in a `file:` URI, a second open path to keep in step; or falling back to memory on any install error, which would hide decision 4's case of a second tab.
- Topics: browser, wasm, sqlite, view, rfc-0006
