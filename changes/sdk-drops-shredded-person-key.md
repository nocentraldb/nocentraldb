---
type: feature
tickets: [NOC-229, NOC-224]
components: [core, sdk-rust]
---
## Problem
A member kept a person key in memory after the key was shredded, until the database closed, so an erasure did not reach the key on a member that stayed open.

## Use
Nothing to call. As soon as a member folds a shred, from the relay or from its own batch, it drops the shredded person key from memory and never holds it again (RFC 0008, "A shred is final when accepted"). The core no longer computes a shred's stability.
