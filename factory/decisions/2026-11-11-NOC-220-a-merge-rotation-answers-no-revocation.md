# 2026-11-11 · NOC-220 · A merge rotation answers no revocation, the one after a leave included

- Decided by: pm (NOC-220)
- Why: RFC 0008's key chain entry has key 10, `answers`, "the revocations this rotation answers", and step 5 of "Two removals that rotate the key at once" says nothing about it for a merge rotation. The core's `MergeRotation::entry` (membership/current.rs) already writes it empty ("A merge rotation answers no revocation"), and neither the relay nor the fold reads the field. The SDK's merge rotation follows the core: `answers` stays empty, also when the merge rotation is the one that rotates away from a leave.
- Instead of: answering every remove or leave whose subject may hold the current generation, which would be a new rule and a change of the core's comment.
- What this does not do: nothing records which leave a merge rotation answered; a later rule that needs it becomes its own RFC question.
- Topics: membership, rfc-0008, sdk-rust
