# 2026-11-16 · NOC-354 · The bridge reads `{ space, id }` as a reference by the field's type in the lock

- Decided by: coding agent, NOC-354
- Why: RFC 0009 has TypeScript write a reference as the `{ space, id }` a read gives, or `null`; the TypeScript SDK already sends it as that map, so only the bridge changes. It reads the lock for the field's type, as it does for floats and links (NOC-353), and takes a map of exactly `space` and `id`, each 32 hex digits; anything else is `invalidValue`, with a text of its own that names the form. A read answers a reference in the same form, so a write takes what a read gives.
- Instead of: a tagged map such as `{ $ref: [space, id] }` beside `$add` and `$remove`, which would change the protocol for every write and make the SDK know each field's kind; and a map with keys beyond `space` and `id`, which is refused rather than ignored, so a mistyped key never writes a reference the app did not mean.
- Topics: bridge, sdk, typescript, references
