# 2026-11-14 · NOC-317 · The TypeScript view report is Rust's ViewStatus in camel case

- Decided by: Carsten, 2026-11-14, on NOC-317 (A)
- Why: RFC 0009 names `db.view.current()` and `db.view.subscribe()` for TypeScript but not their value. It is `{ rebuild: { folded: number, total: number } | null }`, Rust's `ViewStatus` (NOC-227) in camelCase, with `rebuild` null while no rebuild runs; RFC 0009's "Sync and status" says so.
- Instead of: a shape of its own for TypeScript (option B).
- Topics: sdk-ts, view, rfc-0009
