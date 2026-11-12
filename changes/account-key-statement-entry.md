---
type: feature
tickets: [NOC-233]
components: [core, relay]
---
## Problem
After a lost device, the account's recovery key names a new account key (RFC 0010). Until now only an honest SDK refused the old key. A space did not learn the newer key, so a thief holding the old account key could still certify a device and add it, or sign off an erasure.

## Use
Nothing to call yet. The core and the relay read the account key statement entry, control entry type 7 (RFC 0008, "Account key statement entry"). Once a space holds a newer statement for an account, the fold refuses an add whose certificate names the older account key, an add written concurrently with the statement, wherever it falls in the order, and an erasure signed off by it. A member admitted under the older key stays a member; removing the lost device is still a remove.
