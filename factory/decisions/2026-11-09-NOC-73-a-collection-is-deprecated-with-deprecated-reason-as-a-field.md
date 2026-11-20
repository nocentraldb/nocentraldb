# 2026-11-09 · NOC-73 · A collection is deprecated with .deprecated(reason) on nc.collection, as a field is, and cannot be undone

- Decided by: Carsten (RFC 0005 decision D17)
- Why: one name for one idea: a developer who knows `.deprecated()` on a field finds it on a collection, and the lock reads the same in both places, with `deprecated`, the model version that deprecated it, and a new collection key `deprecatedReason`. The SDK hides the collection from its TypeScript types and refuses writes into it with `deprecated`.
- Instead of: an option in `nc.collection`'s second argument, `{ deprecated: "reason" }`, which spells the same thing a second way; no deprecated collections in v1, which leaves a given-up collection in every type and lets new objects keep appearing in it.
- Topics: rfc-0005, schema, model-lock, sdk
