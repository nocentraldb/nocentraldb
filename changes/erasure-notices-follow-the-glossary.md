---
type: fix
tickets: [NOC-359]
introduced: cycle
components: [sdk-rust, sdk-ts]
---
## Symptom
The notice `erasurePending` said "Personal data was erased", where an erasure makes personal fields unreadable. The notice `takenBack` used "finalize" as a noun ("a finalize another device of the account wrote").

## When
On every device of the account while an erasure is provisional (`erasurePending`), and when an erasure is taken back (`takenBack`).

## Cause
Both texts were written before `docs/glossary.md` set the terms.

## Fix
`erasurePending` says "Personal fields were erased on a device of this account", and `takenBack` says "an erasure that another device of the account made final before it saw this take-back still wins". RFC 0009 quotes both texts and follows. The notice codes are unchanged.
