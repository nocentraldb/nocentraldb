# 2026-11-12 · NOC-254 · Step 1 keeps reading the past state as `Resolved`

- Decided by: coding agent (NOC-254)
- Why: `judge` passes the past state on to `rights::own_rules` and `rights::signed_off`, which read about a dozen of its methods (generations, roles, members, certificates, person keys, statements); a narrower trait would repeat nearly all of them, and `rights` would still depend on `resolve`, so it only adds indirection.
- Instead of: a trait or a struct of the few values step 1 reads, so that `judge` and `rights` would not depend on `resolve`.
- Topics: membership, core, layering
