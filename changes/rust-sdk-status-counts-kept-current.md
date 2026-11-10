---
type: internal
tickets: [NOC-147]
components: [core, sdk-rust]
---
The Rust SDK's sync status reads the unconfirmed and refused counts the core's log keeps per author, instead of decoding the whole log on every read.
