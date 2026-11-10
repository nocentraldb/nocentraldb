# 2026-11-10 · NOC-71 · The membership verdict for sealed changes follows the generation graph

- Decided by: coding agent (NOC-71)
- Why: RFC 0008 ("Sealed changes") counts a sealed change under a grant when its key generation is the grant's from generation "or descends from it in the generation graph", which NOC-78 left out until the graph existed. `Resolved::admits` now asks `Resolved::descends(key generation, from generation)` on the graph of the whole set, with the same cutoff rule as before. A generation that is not in the graph descends from nothing, so a change under it is "not a member".
- Instead of: NOC-78's interim rule that a key generation counts only when it is the grant's own from generation, under which a writer's changes after a rotation are rejected.
- Topics: rfc-0008, membership, relay, keys
- Supersedes: 2026-11-09-NOC-78-step-4-uses-resolved-admits-with-the-grant-s-own-generation.md
