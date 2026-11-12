---
type: internal
tickets: [NOC-266]
components: [sdk-rust]
---
The Rust SDK creates the account space with the device's first app space and keeps the device's member and membership records there; `spaces().list()` leaves it out.
