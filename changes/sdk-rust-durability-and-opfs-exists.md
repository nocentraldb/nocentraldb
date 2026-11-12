---
type: internal
tickets: [NOC-175]
components: [sdk-rust, core]
---
The Rust SDK counts its own changes the relay has not confirmed for a crate-internal durability report, and in the browser it refuses to draw new keys for a database on OPFS that exists without them; the core tells whether a database file exists on OPFS.
