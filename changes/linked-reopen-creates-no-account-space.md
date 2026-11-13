---
type: fix
introduced: cycle
tickets: [NOC-311]
components: [sdk-rust]
---
## Symptom
A database linked into the person's account could create a second account space for that account.

## When
It was opened again before the person's account space had reached it, then created a space or paired a device.

## Cause
After a reopen, nothing told the database that it had joined an account another device created.

## Fix
While the database still keeps its link statement, which it does until the person's account space holds it, it creates no account space. A device paired without a link of its own still has the same window; that part waits for the owner.
