---
type: feature
tickets: [NOC-234]
components: [core, relay, sdk-rust]
---
## Problem
In a space whose members all belong to one account, no second account can sign off an erasure, so nobody could erase a person's data there. RFC 0008 lets the performer's own account sign off in that case, provisionally for 7 days, so that a stolen device cannot destroy a person's key at once.

## Use
Nothing to call yet. The core reads the erasure decision, control entry type 8 (RFC 0008, "Erasure decision"). In a one-account space, an erasure signed off by the performer's own account is provisional: the person shows as erased, but members keep the person key and the relay keeps its record. An admin of that account finalizes it, after which members destroy the key and the relay deletes the record, or takes it back, which shows the person's data again. A finalize wins over every concurrent take-back. The relay's 7-day gate and the SDK's status, notice and automatic finalize follow.
