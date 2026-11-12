---
type: internal
tickets: [NOC-230]
components: [sdk-rust]
---
The Rust SDK knows RFC 0007's new code `too_early`, which the relay sends when an erasure decision would finalize a provisional erasure less than 7 days after its shred. The sync status and a refused change show it as the relay sent it, instead of `unknown_error`.
