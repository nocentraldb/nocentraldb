---
type: internal
tickets: [NOC-170]
components: [sdk-rust]
---
The Rust SDK's crate documentation cites RFC 0009's section on where the database lives instead of a removed decision id, and a repository test now fails on any citation of an id its RFC no longer defines. No behaviour changes.
