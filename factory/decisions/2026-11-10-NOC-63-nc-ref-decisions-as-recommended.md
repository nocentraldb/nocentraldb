# 2026-11-10 · NOC-63 · RFC 0004 Amendment 4 and RFC 0005 Amendment 5: every nc.ref decision as recommended

- Decided by: Carsten
- Why: the owner accepted every recommendation in PR #92, in chat with the coding agent (Carsten, 2026-11-10). RFC 0004's decisions 15 to 20, all A: the type `ncdb/ref` written by set (1); the value `[space id, object id]`, two 16-byte byte strings; a value that does not fit reads missing; a target in an unlisted collection reads missing; a reference into its own space is allowed; a target is not accessible when the member holds no copy of its space. RFC 0005's D18 to D22, all A: `nc.ref("collection")` or a list, with the lock key `targets`; one 32-byte BLOB index column; reads give `{ space, id }` and `space.inspect` gives `"ok"`, `"notAccessible"` or `"missing"`; subscriptions do not follow the state; `.describe` and `.deprecated` only, no personal reference and no `nc.refs` in v1.
- Instead of: the other options each decision lists.
- Topics: rfc-0004, rfc-0005, references, schema
