---
type: feature
tickets: [NOC-120]
components: [core, sdk-rust]
---
## Problem
A change the relay refused because of a revocation was never pushed to that relay again, even after a concurrent change made the revocation ineffective and every member accepted the change. Members that sync only through the relay never received it.

## Use
Nothing to call. The Rust SDK pushes such a change once more as soon as its own fold accepts it again (RFC 0008, M19). If the relay accepts it, `db.sync().current().refused` no longer counts it. If the relay refuses it again, it stays refused until another control change makes the fold accept it again.
