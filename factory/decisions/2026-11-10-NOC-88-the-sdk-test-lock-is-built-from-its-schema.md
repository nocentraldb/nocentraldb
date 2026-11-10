# 2026-11-10 · NOC-88 · The Rust SDK's test lock is built from its schema, and the fixture check holds it

- Decided by: coding agent (NOC-88)
- Why: NOC-85 merged while this PR was open, so `nc.index` takes `scope: "space"` and `ncdb model build` writes it. The lock of `crates/nocentraldb/tests/model/model.ts` is now what the build writes, and `crates/ncdb/tests/fixtures.rs` checks it like the view's test lock.
- Instead of: keeping the scope added by hand, which no check would hold.
- Topics: sdk-rust, model-lock, indexes, tests
- Supersedes: 2026-11-10-NOC-88-the-sdk-test-lock-gets-its-space-scope-by-hand.md
