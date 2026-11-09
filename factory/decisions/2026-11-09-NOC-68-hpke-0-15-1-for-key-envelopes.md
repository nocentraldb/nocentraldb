# 2026-11-09 · NOC-68 · Key envelopes use hpke =0.15.1 with only alloc, x25519 and chacha

- Decided by: coding agent (NOC-68)
- Why: RFC 0003 names the `hpke` crate and leaves the pin to the implementation ticket. 0.15.1 is the current release; its line (0.14 and up) runs on the same `chacha20poly1305 =0.11.0`, `hkdf =0.13.0` and `sha2 =0.11.1` the core already pins, and 0.15 pulls the security fixes its MSRV bump names. Without default features it brings no ML-KEM, NIST curves or AES. Every 0.14+ release builds on `x25519-dalek` 3, which brings `curve25519-dalek` 5 beside the 4.1.3 that `ed25519-dalek` 2.2 needs; that second line is the price of the crate RFC 0003 names. No `getrandom` feature: the core draws the ephemeral key's 32 input bytes with its own `getrandom` and hands them to HPKE's DeriveKeyPair, which is also how RFC 9180's ikmE vectors are reproduced. The crate has no paid audit (RFC 0003, decision 3); RFC 9180's vector A.2.1 runs in the gate, natively and in wasm32.
- Instead of: 0.13.0 (on `chacha20poly1305` 0.10, `hkdf` 0.12 and `sha2` 0.10, a second RustCrypto line), or `hpke-rs`, which RFC 0003 rejected.
- Topics: crypto, dependencies, rfc-0003
