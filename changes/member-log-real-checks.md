---
type: internal
tickets: [NOC-48]
components: [core]
---
`ncdb-core`'s member log checks received changes with the real signature and decryption: `SealChecks` verifies with `seal::verify` and decrypts with `seal::open`, and every `OpenError` maps to the step and reason of RFC 0002 Amendment 1, natively and in wasm32.
