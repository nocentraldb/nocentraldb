---
type: feature
tickets: [NOC-229, NOC-224, NOC-228]
components: [core, relay]
---
## Problem
An erasure waited until every admin device had written after it, so one lost admin device kept a person's key alive indefinitely. Even then, an admin device that was stolen and not yet removed could still reject the erasure after its key was destroyed. A team whose only admin performs the erasure had no second account that could approve it.

## Use
Nothing to call. A shred is final as soon as a member or the relay accepts it: no revocation rejects it later, and the relay deletes the shredded key's record when it accepts the shred (RFC 0008, "A shred is final when accepted"). When the performer's account is the only account with an admin member, another account's writer member can sign off the erasure; a reader's account never can.
