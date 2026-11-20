# 2026-11-10 · NOC-63 · nc.ref takes one collection or a list, locked as targets, with only .describe and .deprecated and no nc.refs

- Decided by: Carsten, 2026-11-10, on PR #92 (RFC 0005 decisions D18 and D22)
- Why: `nc.ref("collection")` or a list, written in the lock as `{ "type": "ncdb/ref", "rule": 1, "targets": [...] }` with links' key `targets`; adding a target collection is accepted and removing one refused, as for links, so a reference reads like a link and the only new thing is that the target may live in another space. A reference cannot be personal or the `person` of a personal field, because a person in another space has no key here a shred could destroy; many references can come later as a type of their own.
- Instead of: one collection only, or `nc.ref()` to any object (D18); `nc.refs("collection")`, a set of references that could not be indexed in v1 (D22).
- Topics: rfc-0005, references, schema, model-lock
