---
type: internal
tickets: [NOC-217]
components: [sdk-rust]
---
The reopen-after-restore test of the Rust SDK waits until the SDK stops reporting the new relay epoch, instead of reading it right after the relay confirmed every change, which made it fail now and then under load.
