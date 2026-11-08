# 2026-11-08 · NOC-13 · RFC 0007 follows RFC 0008 on membership, cutoffs and person-key records

- Decided by: coding agent (NOC-13), carrying out RFC 0008's Conflicts points 11 and 12 and the owner's decisions M3, M19 and M20 (Carsten, 2026-11-08)
- Why: the relay must reach the same verdict as the members. So RFC 0007 now takes RFC 0008's person-key entry (commitment, generation and record hash), a membership table that is RFC 0008's `resolve` (grants with from generations, revocations with cutoffs, the generation graph), the cutoff check on a sealed change's author sequence, record replacement by descendant generation, and the deletion of a record only once its shred is stable. R7 is decided by M3. The refusal rows of the error table follow M19, and "What the relay sees" adds RFC 0008's list (M11), so that RFC 0007 does not contradict a decided RFC.
- Instead of: keeping RFC 0007's own record hash, its positions of addition and removal, deletion in the append's transaction and R7 A by the relay's order, which RFC 0008 replaced.
- Topics: relay, rfc-0007, rfc-0008, membership
