# 2026-11-08 · NOC-30 · The bytes of a sealed change: content key, nonce, associated data, padding and crates

- Decided by: Carsten
- Why: the owner accepted option A on all five details RFC 0003 left open, as pm recommended: the content key is HKDF-SHA256 of the space key with no salt and the fixed-width info `"nocentraldb/content-key" ‖ space id (16) ‖ generation (8, big-endian) ‖ suite (8, big-endian)`; header key 6 is a random 24-byte nonce ‖ ciphertext ‖ tag; the associated data is the canonical header without key 6, extension keys included; the whole encoded payload map, key 5 included, is a multiple of 256 bytes, and a payload already on a multiple gets no key 5; the crates are `chacha20poly1305 =0.11.0` and `hkdf =0.13.0` with `sha2 =0.11.1`. RFC 0003 records them as Amendment 1 (PR #47).
- Instead of: for each detail, options B and C in NOC-30, listed one line each in RFC 0003 Amendment 1.
- Topics: crypto, wire-format, rfc-0003, dependencies
- Supersedes: 2026-11-08-NOC-15-four-sealing-details-rfc-0003-leaves-open-built-as-proposed.md
