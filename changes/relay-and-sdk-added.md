---
type: internal
tickets: [NOC-153]
components: [relay, sdk-rust]
---
When an append makes a member current in a space, the relay server sends `added {space}` to that member's open connections (RFC 0007, decision R13), and the Rust SDK pulls, pushes and subscribes to the space at once, without reconnecting; an `added` for a space it had stopped after `not_member` or `removed_member` syncs that space again.
