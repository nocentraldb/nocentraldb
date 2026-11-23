---
type: feature
tickets: [NOC-469]
components: [relay]
---
## Problem
An app with open signup could not let its users create spaces on a relay without the operator listing every account and redeploying.

## Use
The operator sets `NCDB_ADMISSION_KEYS` to the app's public admission key, 64 hex digits (the AWS stack does not set it yet). A member presents an admission token that the app's backend signed with `admit`, and its account then creates spaces up to the admission's `spaces` until it lapses. The SDKs do not send `admit` yet.
