# 2026-11-08 · NOC-7 · Ed25519 signs, HPKE with X25519 wraps keys, XChaCha20-Poly1305 encrypts, and v1 is not post-quantum

- Decided by: Carsten (RFC 0003 decisions 2, 3, 4 and 9, and reading 6)
- Why: Ed25519 signatures are deterministic and behave the same on every platform, while the platform keystore still protects the key at rest; XChaCha20-Poly1305's random nonces are safe for many writers and fast in WebAssembly; HPKE is the IETF standard for encrypting to a public key, and the `hpke` crate, which has no paid audit, runs the RFC 9180 test vectors in CI and gets a paid audit of our combination before 1.0; the Rust implementations of ML-KEM are young, so v1 ships X25519, reserves a suite id for a hybrid suite, and the documentation says plainly that v1 is not post-quantum (principle 4).
- Instead of: AES-256-GCM; ECDSA P-256 held in hardware where the platform offers it; the `hpke` crate without the audit, or `hpke-rs`; a hybrid X25519 + ML-KEM key agreement from v1.
- Topics: rfc-0003, crypto, keys, dependencies

The constructions, crates, audits and suite id are in `docs/rfc/0003-keys-and-cryptography.md`, "Primitives" and "Not post-quantum in v1". The paid audit before 1.0 costs money, which GENESIS.md reserves for the owner; it is part of this decision.
