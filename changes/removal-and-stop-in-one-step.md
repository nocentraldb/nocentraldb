---
type: fix
tickets: [NOC-402]
introduced: cycle
components: [sdk-rust]
---
## Symptom
Right after the relay told a member it was removed from a space, the member could list the space without a role while `sync().current().error` still read `None`.

## When
Between the moment the SDK folded the removal the relay's `not_member` or `removed_member` error carried and the moment it recorded that code, a short window another thread could read in.

## Cause
The SDK folded the removal and recorded the code under two separate locks of the database.

## Fix
The SDK folds the removal and records the code under one lock, so the role and the sync status change together.
