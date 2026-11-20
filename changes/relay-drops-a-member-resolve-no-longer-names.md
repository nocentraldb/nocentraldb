---
type: fix
tickets: [NOC-390]
introduced: cycle
components: [relay]
---
## Symptom
The relay kept serving a member whose add a later removal had excluded: it accepted the member's uploads, served it pulls and listed the space for it, and still admitted changes it had written before.

## When
An admin adds a member, and another admin then removes that admin with a cutoff below the add, so the add no longer counts and the added member is no member at all.

## Cause
After a control change, the relay rewrote the membership rows only of members that `resolve` of its log still names. A member that dropped out of `resolve` entirely kept its old row.

## Fix
The relay rewrites the row of every member that a control change drops out of `resolve` as a non-member that admits no change, so its membership table equals `resolve` of its log. Both the local and the AWS relay get the fix.
