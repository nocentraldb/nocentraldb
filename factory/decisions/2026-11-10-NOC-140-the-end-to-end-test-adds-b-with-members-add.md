# 2026-11-10 · NOC-140 · The end-to-end test adds device B with `members().add`

- Decided by: coding agent (NOC-140), as NOC-91's decision foresaw
- Why: `db.invitation()` and `space.members().add` exist now, so `crates/nocentraldb/src/sync/tests/two_devices.rs` adds device B through the public API, and its own helper that wrote the entries through the crate's internals is gone. A test of its own (`sync/tests/members.rs`) adds a member of another account with its fingerprint, which the end-to-end test, with both devices of one simulated account, does not exercise.
- Instead of: keeping the internal helper next to the public call.
- Topics: sdk-rust, rfc-0009, testing
- Supersedes: 2026-11-10-NOC-91-the-end-to-end-test-adds-b-through-the-sdks-internals-until-noc-140.md
