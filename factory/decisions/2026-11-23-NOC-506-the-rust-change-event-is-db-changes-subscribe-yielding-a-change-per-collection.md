# 2026-11-23 · NOC-506 · The Rust change event is `db.changes().subscribe()`, a stream of one `Change` per fold and collection, with `Origin::Local` or `Origin::Remote`

- Decided by: the coding agent of NOC-506, where RFC 0009 names no Rust form; for the owner's review in the PR
- Why: it follows RFC 0009's Rust rules: the TypeScript names in snake case, a handle like `db.view()` and `db.sync()` whose `subscribe()` returns a `Stream`, and one item per collection, as TypeScript calls its listener once per collection; `Change` and its field names are TypeScript's, and `Change` is `#[non_exhaustive]` like the SDK's other report structs
- Instead of: one item per fold that lists its collections, which TypeScript does not have; a `db.subscribe_changes()` call with no handle, which breaks the pattern of `db.changes.subscribe`
- Topics: sdk-rust, subscribe, naming, rfc-0009
