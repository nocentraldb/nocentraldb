---
type: internal
tickets: [NOC-136]
components: [sdk-rust]
---
In the Rust SDK, a write from a reader fails with `NotAllowed`, and one from a member whose copy folded its removal fails with `NotMember`, before any change is made (RFC 0008, "Roles and rights"; RFC 0009, "Errors").
