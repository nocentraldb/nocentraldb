---
type: internal
tickets: [NOC-263]
components: [core]
---
`ncdb-core` reads and writes the recovery code as twelve BIP-39 English words (RFC 0010, "The recovery code"), derives the recovery key from it (RFC 0003, "Recovery key"), signs the first account key statement and computes the fingerprint over the recovery key, pinned by vectors in docs/rfc/vectors/0010/.
