# 2026-11-15 · NOC-320 · Only an agent or a sub-agent adds a sub-agent

- Decided by: Carsten, 2026-11-15, on NOC-320 (Q2 a)
- Why: the core accepts a sub-agent's add only when its certificate is signed by a parent whose own certificate is of kind 2 or 3 (RFC 0003, "Member certificate"; RFC 0008). A database of a device that adds a version 3 invitation is refused with `NotAllowed`. Creating an ad-hoc space stays open to every member; only adding the sub-agent needs an agent.
- Instead of: letting a device sign a sub-agent's certificate, which would need a change to RFC 0003.
- Topics: sub-agents, membership
