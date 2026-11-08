# 2026-11-07 · NOC-14 · Debug shows ids and ordering metadata, never content

- Decided by: coding agent (NOC-14)
- Why: GENESIS.md forbids content and keys in logs and errors. `Value` and `Key` show their kind and size; an operation shows only its unknown keys, since RFC 0002 counts object ids, collection and field names, ops and values as content; the sealed payload, padding and signature show only their length. Space, member and change ids, sequence numbers, key generations, formats, kinds, clocks and dependencies show, as hex where they are bytes: the relay or a member sees them without content. Errors name keys and byte offsets only.
- Instead of: derived `Debug`, which prints every byte, or hiding ids too, which leaves nothing to debug with.
- Topics: logging, privacy
