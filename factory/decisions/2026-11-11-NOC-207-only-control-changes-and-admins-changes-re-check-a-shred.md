# 2026-11-11 · NOC-207 · Only a control change or a change of a member that holds or held admin makes the relay re-check a waiting shred

- Decided by: coding agent (NOC-207), inside the relay core
- Why: a shred's stability (RFC 0008, "When a change is stable") depends on the control changes and on the runs of the members that hold or held the admin role, nothing else. A control change can change membership, revocations and shreds; a sealed change of such a member can close a gap in its run, also one that makes a revocation stable, which then bounds another member. A sealed change of any other member changes none of it. So the relay runs the check of #188 (NOC-113) after a control change, or after a sealed change whose author's row in the membership table holds admin or has an accepted grant of admin; it skips it after every other sealed change. The check itself, and so when a record is deleted, is unchanged.
- Instead of: re-checking only after a change of a member that blocks the shred, which misses a sealed change of a member that blocks only a revocation the shred waits on; keeping the resolved state and the runs incrementally in the relay, which needs new state in every backend.
- What this does not do: in a space with a waiting shred, a control change or an admin's sealed change still costs one log read and one `resolve`.
- Topics: relay, erasure, rfc-0008
