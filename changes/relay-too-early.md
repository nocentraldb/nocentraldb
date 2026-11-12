---
type: feature
tickets: [NOC-234]
components: [relay]
---
## Problem
A provisional erasure in a one-account space becomes final through an erasure decision that finalizes it. Without a wait at the relay, a thief with a device of the account could finalize at once, before the account's owner had a chance to take the erasure back.

## Use
Nothing to call. The relay refuses an erasure decision that finalizes a provisional shred less than 7 days, by its own clock, after it accepted the shred, with the new code `too_early` (RFC 0007, "What the relay checks"). The member keeps the change and pushes it again once the 7 days have passed. A take-back needs no wait.
