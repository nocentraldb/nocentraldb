---
type: internal
tickets: [NOC-90]
components: [sdk-rust, core]
---
The Rust SDK `nocentraldb`, not yet released, syncs with one relay given by `OpenOptions::relay`: it pulls each space from its cursor, pushes every change the relay has not confirmed in local order, subscribes, reconnects with backoff and re-pulls from 0 after a new relay epoch; `db.sync().current()` counts this member's unconfirmed and refused changes, and the core's view records relay positions and refusals in its log.
