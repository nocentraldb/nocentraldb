---
type: internal
tickets: [NOC-158]
components: [sdk-rust]
---
In the Rust SDK, once the relay answers `not_member` or `removed_member` for a space, the member stops syncing it, `spaces().list()` shows it with `role: None`, a write or `members().add` into it fails with `NotMember`, and `SyncStatus::error` reports the code; the member keeps reading its copy (RFC 0007, "The errors"; RFC 0009, "Sync and status").
