---
type: internal
tickets: [NOC-204]
components: [sdk-rust, core]
---
The Rust SDK's key rotation chains every head of the generation graph and is folded with the add that follows in one transaction (`View::receive_all`), so an error or a crash in the middle leaves none of it.
