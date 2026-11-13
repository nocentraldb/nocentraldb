---
type: fix
tickets: [NOC-287]
introduced: cycle
components: [sdk-rust]
---
## Symptom
A guest, a device of an account the relay does not list, that created a space had its account space refused, so the one space the relay allows such an account went to an app space instead.

## When
On a relay that admits only listed accounts, when a guest's device created its first space.

## Cause
`spaces().create()` created the app space before the account space, and the sync pushed spaces in the order of their ids, so the relay saw the app space's genesis first.

## Fix
The account space is created first, and the sync pushes it before any other space.
