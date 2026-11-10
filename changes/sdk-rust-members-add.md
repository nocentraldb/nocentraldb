---
type: internal
tickets: [NOC-140]
components: [sdk-rust]
---
The Rust SDK's `db.invitation()` returns the member's `Invitation`, and `space.members().add(&invitation, AddMember { role, fingerprint })` adds it to a space as RFC 0009 describes, refusing an account new to the database without its fingerprint with `UnknownAccount`.
