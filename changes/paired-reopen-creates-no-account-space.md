---
type: fix
introduced: cycle
tickets: [NOC-311, NOC-312]
components: [core, sdk-rust]
---
## Symptom
A device paired into an account could create a second account space for that account.

## When
It was opened again before the account's account space had reached it, then created a space or paired a device.

## Cause
After a reopen, nothing told the device that it had joined an account another device created.

## Fix
A device that completes a pairing keeps that it joined beside its log, and creates no account space until the account's reaches it.
