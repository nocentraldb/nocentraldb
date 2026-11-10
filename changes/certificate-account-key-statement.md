---
type: internal
tickets: [NOC-191, NOC-186]
components: [core]
---
`ncdb-core` implements RFC 0003's account formats as the owner decided them in NOC-186: the member certificate carries the account key statement as key 6, outside key 5's signature (a sub-agent's carries its parent's account's), the recovery key derives from the recovery code with HKDF-SHA256, and a pairing answer seals the account key under the info label alone; byte-exact vectors from Python's cryptography run natively and in wasm32, and a certificate without key 6 verifies as before.
