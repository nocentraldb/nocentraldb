---
type: security
tickets: [NOC-196, NOC-201, NOC-132]
components: [core]
---
## Description
An erasure needs a sign-off by an admin of another account than the performer's. The fold now also treats two account keys as one account when any certificate in the shred's causal past, of a current or a former member, the performer's included, attaches the same recovery key to both. A statement in a certificate can therefore only block a sign-off, never enable one, and two account keys of one person after a recovery cannot sign off each other's erasures once a certificate links them. Without statements, the fold compares account keys as before.

## Advisory
pending
