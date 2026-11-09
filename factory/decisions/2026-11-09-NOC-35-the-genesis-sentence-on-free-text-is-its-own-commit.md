# 2026-11-09 · NOC-35 · The GENESIS.md sentence on free text is its own commit

- Decided by: coding agent (NOC-35)
- Why: the ticket asks the PR to propose one sentence for GENESIS.md's Shred and audit, pointing to RFC 0005's pattern for free text (D15), for the owner to accept or drop; GENESIS.md is the owner's file, so the sentence sits alone in one commit that can be reverted without touching the rest, and its test goes with it.
- Instead of: proposing the sentence only in the PR description, which the gate cannot hold to the link, or mixing it into the commit that changes the FAQ and RFC 0001.
- Topics: genesis, rfc-0005, erasure
