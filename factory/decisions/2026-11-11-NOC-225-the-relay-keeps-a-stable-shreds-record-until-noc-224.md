# 2026-11-11 · NOC-225 · The relay keeps a shredded key's record once the shred is stable, until NOC-224 is settled

- Decided by: pm (NOC-225), built by the coding agent
- Why: NOC-224 found that a shred that is stable can still be rejected later by an admin key that was not yet revoked. The relay's deletion of the person-key record once the shred is stable (#188, NOC-113) is the only effect it cannot undo, so it is switched off: the relay still checks stability after each append that can change it, and still refuses a new record of a shredded id, but deletes nothing. The RFC text is not changed in this step.
- Instead of: keeping the deletion and fixing the stability rule first, which waits for NOC-224's answer while a deletion that may be wrong stays live.
- What this does not do: a shredded key's record stays stored, and readable by current members, until NOC-224 settles when it may go.
- Topics: relay, erasure, rfc-0008
