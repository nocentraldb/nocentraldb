# 2026-11-27 · NOC-568 · The core resolves a reference by the space of the object that holds it, with a link's statuses in the reference's status

- Decided by: coding agent (NOC-568)
- Why: Whether a reference is in-space depends on its value and its object's space, so the caller passes that space, and one status type keeps the five link statuses for in-space and the three states for cross-space references.
- Instead of: A second resolve function for in-space references, or returning a link status for them and a reference status for the others.
- Topics: rfc-0018, references, links, core
