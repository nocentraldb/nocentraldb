# 2026-11-08 · NOC-15 · Four sealing details RFC 0003 leaves open are built as proposed, pending the owner (NOC-30)

- Decided by: coding agent (NOC-15); pm put them to the owner in NOC-30, since GENESIS.md reserves wire formats for the owner
- Why: RFC 0003 names the primitives and the order, not these bytes. As built: (1) the content key is HKDF-SHA256 of the space key, no salt, info `"nocentraldb/content-key" || space id (16) || key generation (8, big-endian) || suite id (8, big-endian)`, the shape of RFC 0003's key-chain subkey; (2) header key 6 is the 24-byte nonce, then the ciphertext with its 16-byte tag (libsodium's layout); (3) the associated data is the canonical header map without key 6, unknown keys included, so a later header field is bound too; (4) the canonical payload map, padding key 5 included, is a multiple of 256 bytes, and a payload too few bytes short for key 5 (1 or 26 bytes short) pads to the multiple after. All four sit in `src/seal.rs` and `generate.py`; a different answer changes them and the vector.
- Instead of: guessing silently, or waiting with the whole ticket for the answer.
- Topics: crypto, wire-format, rfc-0003
