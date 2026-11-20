# 2026-11-16 · NOC-353 · The bridge reads links and references by the field's type in the lock, as it reads floats

- Decided by: coding agent, NOC-353
- Why: RFC 0009 has TypeScript write a link as the target's object id, a `string`, so the protocol carries it as text; the bridge already reads the lock for float fields, and it reads it the same way for `ncdb/link` and `ncdb/links`, so the TypeScript SDK and the protocol stay as they are. A text that spells no object id is `invalidValue`, as an id elsewhere in the protocol is. A reference field takes a map of exactly `space` and `id`, each 32 hex digits, the form a read answers (NOC-354); anything else is `invalidValue`, so a mistyped key never writes a reference the app did not mean.
- Instead of: a tagged value from the TypeScript SDK, such as CBOR bytes or `{ $id }`, which changes the protocol for every write and makes the SDK know each field's kind as the bridge already does.
- Topics: bridge, sdk, typescript, links, references
