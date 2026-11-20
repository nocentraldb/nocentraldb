# 2026-11-08 · NOC-37 · A new field operation takes a new rule version or a new field type, and a number never used before

- Decided by: Carsten (RFC 0004 Amendment 1, decision 13)
- Why: a merge rule never changes under the same type id and rule version, as RFC 0004's contract says. Until the migrations RFC exists only new fields can use a new rule version, and an old client parks the unknown number instead of misreading it. A new argument that changes state, read or outcome follows the same rule.
- Instead of: adding operations to a type under the same rule version as long as existing ones keep their meaning: faster to extend, but an old and a new client then differ under the same version.
- Topics: rfc-0004, field-types, plugins, wire-format

See `docs/rfc/0004-merge-rules-basic-types.md`, "How a field type adds operations".
