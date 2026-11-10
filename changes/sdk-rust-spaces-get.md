---
type: internal
tickets: [NOC-119]
components: [sdk-rust, core]
---
The Rust SDK's `spaces().get(id)` returns the handle of a space the member created or joined through sync, whose writes depend on the heads of the member's log; `SpaceOfAnotherModel` and `NotFound` follow RFC 0009's error table.
