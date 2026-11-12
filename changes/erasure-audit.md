---
type: feature
tickets: [NOC-246]
components: [core, sdk-rust]
---
## Problem
After a person's data was erased, an app could not show whether every device that could have held the person's key has deleted it, or which one never answered.

## Use
`space.erasures().list().await?` returns one `Erasure` per shred of the space: its `shred` id, its `state` (`Final`, `Provisional` or `TakenBack`), the `performer` and its account, the account that signed it off, and every member of the shred's causal past, plus any later acknowledger. Each member carries `current`, and `acknowledged` as `Deleted`, `Seen` or `Missing` (RFC 0009, "Erasures"). Every member may call it, a reader too.
