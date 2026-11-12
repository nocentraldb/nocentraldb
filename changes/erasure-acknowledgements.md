---
type: feature
tickets: [NOC-238]
components: [sdk-rust]
---
## Problem
A member that accepted an erasure left no trace of it in the log, so an audit could not tell which members had seen the erasure and deleted their copy of the key.

## Use
Nothing to call. When a member folds a shred it received, the SDK writes its two acknowledgements for it, "seen" and then "deleted", and syncs them (RFC 0008, "Acknowledgement"). Every member writes them, a reader too, once per shred; offline they wait in the log and are pushed later.
