# 2026-11-10 · NOC-95 · In TypeScript a space id is 32 lowercase hex digits, and a reference is written as the { space, id } a read gives

- Decided by: Carsten, 2026-11-10, on NOC-95 (RFC 0009 decisions S23 and S24)
- Why: One rule for object ids and space ids, so a reference reads `{ space, id }` in one form; a write takes what a read gives, or `null` to clear it.
- Instead of: another form of space id (S23 B); a read object of the target, from which the SDK takes `$space` and `$id`, shorter for an app that holds the target but making a write depend on a read of another space (S24 B).
- Topics: rfc-0009, sdk-ts, ids, references
