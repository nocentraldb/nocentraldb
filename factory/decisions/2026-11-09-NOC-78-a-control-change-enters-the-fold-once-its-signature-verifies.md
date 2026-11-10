# 2026-11-09 · NOC-78 · A control change enters the fold once its signature verifies

- Decided by: coding agent (NOC-78)
- Why: `SealChecks` no longer takes a `Membership` trait or keys through `add_member` and `add_space_key`; everything it knows comes from control changes. RFC 0008 says `resolve` is a function of the whole set of control changes and that a verdict can flip either way as more arrive; RFC 0002 Amendment 1 lets a control change stop after step 4 and leaves its meaning to RFC 0008. So `SealChecks` passes a control change at step 4 once step 3 passed (its author's keys come from a held genesis or add entry, or from the change itself for a genesis), adds every such change to the set it folds, and keeps the change's verdict in `SealChecks::resolved`, not in the log state. A control change by an author no held entry names stays rejected as "unknown author" and passes when that entry arrives (D11).
- Instead of: passing only control changes that `resolve` accepts, which leaves a rejected one out of the set, so that two members holding the same changes could fold different sets depending on the order of arrival.
- Topics: rfc-0002, rfc-0008, membership, log, convergence
- Supersedes: 2026-11-08-NOC-48-sealchecks-takes-membership-through-a-trait-and-keys-through-two-methods.md
