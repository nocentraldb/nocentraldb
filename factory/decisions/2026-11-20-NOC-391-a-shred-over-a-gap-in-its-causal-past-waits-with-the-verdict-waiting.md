# 2026-11-20 · NOC-391 · A shred over a gap in its causal past waits with the verdict Waiting, and the relay refuses it with membership_rule

- Decided by: coding agent (NOC-391), for the owner's review on the PR
- Why: RFC 0008 gives a shred its verdict once the holder holds its whole causal past; until then `resolve` gives a shred, or an erasure decision that finalizes, `Verdict::Waiting`, which applies nothing, so no key is destroyed and no record deleted; the relay answers it with its existing code for a change `resolve` does not accept
- Instead of: judging the shred from the part of its causal past the holder has (the bug), reusing `Parked`, which means "not understood", or a new relay error code, which would change the wire protocol
- Topics: membership, erasure, relay
