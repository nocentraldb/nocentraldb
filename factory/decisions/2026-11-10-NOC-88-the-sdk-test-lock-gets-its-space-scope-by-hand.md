# 2026-11-10 · NOC-88 · The Rust SDK's test lock gets its index scope by hand, and no fixture check holds it

- Decided by: coding agent (NOC-88)
- Why: `nc.index` in the TypeScript SDK does not carry `scope` yet (RFC 0005 Amendment 3), so `ncdb model build` writes the index `open` of `crates/nocentraldb/tests/model/model.ts` without it. The SDK's tests need an index that runs in one space for `space.find` and `space.subscribe`, so the lock that the build wrote got `"scope": "space"` on that index by hand, and `crates/ncdb/tests/fixtures.rs` does not check this lock, since `ncdb model check` would call it out of date. Once `nc.index` takes `scope`, the lock is rebuilt and the fixture check added.
- Instead of: building the test model in Rust with `ModelLock` values, as the core's tests of NOC-81 do, which would test the SDK on a lock no app can have; or leaving `space.find` untested.
- Topics: sdk-rust, model-lock, indexes, tests
