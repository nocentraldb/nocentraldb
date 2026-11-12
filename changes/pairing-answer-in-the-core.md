---
type: internal
tickets: [NOC-265]
components: [core]
---
`ncdb-core` builds and reads the pairing answer (RFC 0010, "The pairing answer"): `ncdb-pair-1:` and unpadded base64url of its fixed layout, with the certificate, the account key statement, for a device the account key sealed with HPKE under `"nocentraldb/account-key"`, and the account key's half of the link statement under `"nocentraldb/account-link"`. Completing an answer checks the certificate against the member's own id, the statement against its recovery key, the certificate against the account key the statement names, and that a device's account key opens and matches. Vectors in docs/rfc/vectors/0010/ pin it.
