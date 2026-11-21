# 2026-11-07 · NOC-8 · No field type may assume causal delivery

- Decided by: Carsten (RFC 0004 decision 6)
- Why: members exchange changes in island mode, logs are compacted and the convergence harness delivers in random order with duplicates, so every merge rule must converge under any order of arrival. The cost: each field keeps the ids that were replaced, which grow until compaction.
- Instead of: RFC 0002 guaranteeing causal delivery for field types to rely on: smaller state, but one relay or sync bug that reorders changes would make copies diverge.
- Topics: rfc-0004, merge-rule, convergence, field-types

See `docs/rfc/0004-merge-rules-basic-types.md`, "What the merge rules rely on", and contract item C1 in "The plugin contract".
