# 2026-11-21 · NOC-424 · A member resolves control changes with a memory, so step 1's judgement must read only the node's causal past

- Decided by: coding agent (NOC-424)
- Why: `SealChecks` resolved every control change again from scratch after each one it added, folding each node's whole causal past, so a new person key cost the square of the persons in the space; `membership::Resolver` returns exactly `resolve`'s state but keeps each gap-free node's judgement while its causal past is the very same set, which is sound only while `judge` and the fold of a past read nothing outside that past (RFC 0008, "The function", step 1), as the convergence harness's `membership_resolver` test checks against `resolve`
- Instead of: an incremental fold that applies a new change on top of the last state (a second implementation of steps 2 to 5 to keep equal to `resolve`), or skipping the resolve for person-key entries (a special case convergence would have to prove)
- Topics: membership, performance
