# 2026-11-08 · NOC-15 · The crypto crates are the current releases of the lines RFC 0003 names

- Decided by: coding agent (NOC-15)
- Why: `ed25519-dalek =2.2.0` (2.x, as the ticket says; 3.0.0 exists but is a new major), `curve25519-dalek =4.1.3` pinned directly so that the RUSTSEC-2024-0344 fix cannot be resolved away (4.2.0 is yanked), `chacha20poly1305 =0.11.0`, `hkdf =0.13.0` (both on the same RustCrypto generation as `sha2 =0.11.1`), `zeroize =1.9.1`. ed25519-dalek 2.x still brings `sha2` 0.10 for its own hashing; every hash the core computes uses 0.11.1. No `rand_core`: keys and nonces come from `getrandom`, as ids do. The X25519 public key is computed with curve25519-dalek's clamped base multiplication, so no `x25519-dalek` is needed before HPKE.
- Not audited: NCC Group's 2020 review covered the RustCrypto ChaCha20-Poly1305 code of that time; the pinned `chacha20poly1305 =0.11.0` itself has no audit, nor has `hkdf =0.13.0`. pm put this choice to the owner as point 5 of NOC-30.
- Instead of: `chacha20poly1305 0.10.1` and `hkdf 0.12` (the line closest to the 2020 review, but on `sha2` 0.10, a second hash for HKDF), or `x25519-dalek` for one public key.
- Topics: crypto, dependencies
