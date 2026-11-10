# 2026-11-10 · NOC-91 · The end-to-end test adds device B through the SDK's internals until NOC-140

- Decided by: pm, on the coding agent's report
- Why: the ticket has device A add device B through the SDKs, but the Rust SDK has no `db.invitation()` and no `space.members().add` yet (RFC 0009, "Invitations and members"), and their Rust names are the owner's (NOC-135). The test is in the crate (`crates/nocentraldb/src/sync/tests/two_devices.rs`). It writes A's membership entry and key envelope with A's own key through the crate's internals, after the heads of the space's control DAG, and every other step goes through the public API: create, write, get, find, and sync with `ncdb-relay-server` on SQLite. NOC-140 builds `members().add` and replaces the step.
- Instead of: waiting for NOC-140 and the owner's names before the first end-to-end slice is in the gate, or building a public API that RFC 0009 does not name.
- What this does not do: it does not prove that an app can add a member, and the invitation's account fingerprint (`unknownAccount`) is not exercised, since both devices are of one simulated account. B learns of the space only when it connects again (RFC 0007 sends a member its spaces in `welcome` only), so the test cuts and restores the network after the add.
- Topics: sdk-rust, rfc-0009, testing, safe-subset
