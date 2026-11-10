# 2026-11-10 · NOC-86 · How the core reads a key chain entry, with NOC-97's 72-byte encrypted key

- Decided by: coding agent (NOC-86), following the owner's NOC-97 A (Carsten, 2026-11-10)
- Why: RFC 0008 draws entry type 3 but not every reading rule, and each must be one rule on every member and the relay, with one encoding per entry. Keys 8, 9 and 10 are required; a missing or mistyped one is malformed, and a key from 11 to 63 makes the body not understood. Key 9 holds at least one previous generation, since a rotation always follows one; its items are sorted by generation id with none twice, and each is a map of exactly keys 1 (uint) and 2 (bstr), anything else malformed. Key 2 is 72 bytes, the layout the owner chose in NOC-97 (nonce, ciphertext, tag), and any other length is malformed; whether it opens is a local check outside the fold. Key 10 may be empty, for a merge rotation that answers no revocation, and is sorted without repeats like key 3. This restates NOC-71's rules, which kept the encrypted key as bytes of any length while no decided text fixed its layout.
- Instead of: keeping any length, which would let entries into the fold that no member can open; or allowing an empty key 9, which would give a rotation the ordinal 1 of a genesis.
- Topics: rfc-0008, encoding, keys, membership
- Supersedes: 2026-11-10-NOC-71-key-chain-entry-reading-rules.md
