# 2026-11-10 · NOC-71 · A re-grant is valid when written with a fresh generation

- Decided by: coding agent (NOC-71)
- Why: RFC 0008 ("Membership entry", step 1 of "The function") accepts a re-grant only with a fresh generation, and now that key chain entries and envelopes are folded `resolve` can check it. A re-grant is an add whose subject is a former member (it has a member state in the causal past but no role), or a set role that gives a role that writes to a member whose role does not. Its from generation is fresh when the state of its causal past holds a key chain entry for that generation by the re-grant's author, valid when written, and no key envelope to the subject for that generation, whatever that envelope's verdict. "It saw the revocation" needs no check of its own: the subject has no right to write in the state of the causal past only because a revocation or a reader's add is there. A set role from writer to admin keeps the right to write and is no re-grant.
- Instead of: keeping NOC-67's fail-safe, which rejects every re-grant, so a removed member can never come back; or counting only accepted envelopes against freshness, which would let a key that may have been delivered count as fresh.
- Topics: membership, rfc-0008, keys
- Supersedes: 2026-11-09-NOC-67-re-grants-are-rejected-until-key-chain-entries-are-folded.md
