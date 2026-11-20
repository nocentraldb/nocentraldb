# 2026-11-10 · NOC-71 · How the core reads a key chain entry where RFC 0008 leaves a detail open

- Decided by: coding agent (NOC-71)
- Why: RFC 0008 draws entry type 3 but not every reading rule, and each must be one rule on every member and the relay, with one encoding per entry, as NOC-67 decided for the other bodies. Keys 8, 9 and 10 are required; a missing or mistyped one is malformed, and a key from 11 to 63 makes the body not understood. Key 9 holds at least one previous generation, since a rotation always follows one; its items are sorted by generation id with none twice, and each is a map of exactly keys 1 (uint) and 2 (bstr), anything else malformed. The encrypted previous key is kept as bytes of any length: RFC 0003 fixes its cipher but not its byte layout, and whether it opens is a local check outside the fold. Key 10 may be empty, for a merge rotation that answers no revocation, and is sorted without repeats like key 3.
- Instead of: allowing an empty key 9, which would give a rotation the ordinal 1 of a genesis; or fixing the encrypted key at 72 bytes, a layout no decided RFC states.
- Topics: rfc-0008, encoding, keys, membership
