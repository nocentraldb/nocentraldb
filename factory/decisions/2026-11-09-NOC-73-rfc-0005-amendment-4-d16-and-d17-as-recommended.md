# 2026-11-09 · NOC-73 · RFC 0005 Amendment 4: D16 and D17 decided as recommended

- Decided by: Carsten
- Why: the owner accepted both recommendations of RFC 0005 Amendment 4, in chat with pm: `nc.Infer` types an `nc.int` field as a `number`, writes must be safe integers, and a stored value beyond that range reads `undefined` and is named in `$raw` (D16 A); `.deprecated(reason)` chains on `nc.collection(...)` and the lock gives the collection `deprecated` and `deprecatedReason` (D17 A).
- Instead of: `bigint` or `number | bigint` for D16; an option in `nc.collection`'s second argument, or no deprecated collections in v1, for D17.
- Topics: rfc-0005, sdk, schema
