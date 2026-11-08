---
type: internal
tickets: [NOC-15]
components: [core]
---
`ncdb-core` signs and seals a change as RFC 0003 defines it: member keys with the member id, space keys, payloads padded to 256 bytes and encrypted with XChaCha20-Poly1305 under an HKDF-SHA256 content key, Ed25519 signatures checked with `verify_strict`, typed errors per failing step; standard and independent test vectors run natively and in wasm32.
