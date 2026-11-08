---
type: internal
tickets: [NOC-14]
components: [core]
---
`ncdb-core` implements the change and the log of RFC 0002: the canonical CBOR profile, the frame, header, payload and operation maps with their unknown keys, change and operation ids, random object and space ids, the hybrid logical clock and an append-only in-memory log with a cursor; byte-exact test vectors run natively and in wasm32.
