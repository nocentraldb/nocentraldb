# 2026-11-16 · NOC-357 · RFC 0012's spike measures the official build's `opfs-sahpool`, in the CI's Playwright image, and is not merged

- Decided by: coding agent (NOC-357)
- Why: the spike had to show a per-transaction lock and the outbox replay in WebKit without changing shipped code. The official `@sqlite.org/sqlite-wasm` 3.54.1 is already a dependency (NOC-33's probe), has `pauseVfs`/`unpauseVfs` and `opfs-wl`, and its `opfs-sahpool` is the one our Rust `sqlite-wasm-vfs` ports, so a page with a worker measures both mechanisms in an hour; the CI's pinned Playwright image (`v1.63.0-noble`) runs WebKit, which this container has no other way to run. The code stays in RFC 0012's appendix, as RFC 0006's spike stayed in its PR, so no gate runs it and nothing ships.
- Instead of: a spike through the Rust core and `ncdb_core::view::browser::release`, which would have touched shipped code or needed a parallel build of the worker; Chromium only, which would not answer the WebKit question. The cost per transaction of the Rust port stays unmeasured, and RFC 0012 says so.
- Topics: browser, tabs, rfc-0012, spike
