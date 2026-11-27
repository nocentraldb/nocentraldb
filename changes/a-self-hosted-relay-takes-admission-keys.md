---
type: feature
tickets: [NOC-500]
components: [relay]
---
## Problem
A self-hosted relay could admit accounts only by its allowlist. An app with open signup could not admit its own users by admission token on it, as it can on the relay on AWS with `NCDB_ADMISSION_KEYS`.

## Use
Give `ncdb-relay` the public half of your app's admission key, 64 hex digits, once per key:

```text
ncdb-relay --database relay.sqlite --admission-key 3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c
```

The relay then takes the admissions your app's backend signs with that key, and an admitted account creates spaces up to its admission's spaces.
