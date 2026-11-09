---
type: internal
tickets: [NOC-68]
components: [core]
---
`ncdb-core` seals a space key into a key envelope for one member and opens it with HPKE (base mode, DHKEM(X25519, HKDF-SHA256), HKDF-SHA256, ChaCha20-Poly1305), with RFC 0003's info binding space, generation and recipient, and reads and writes the envelope's fields of RFC 0008's entry type 2; RFC 9180's vector and an independent vector run natively and in wasm32.
