# 2026-11-28 · NOC-596 · A pairing request is sealed to one mailbox key per account, derived from the account key and signed by it

- Decided by: Carsten, 2026-11-28, on NOC-596 (option A)
- Why: a new device knows only the account's identity, and the account has no key that receives. Every device derives the same X25519 key pair with HKDF-SHA256 over the account key's seed, under "nocentraldb/pairing-mailbox-key", and HPKE's DeriveKeyPair; the account key signs its public half in a mailbox record that the new device checks against the identity before it seals its request with HPKE base mode. It uses only RFC 0003's primitives in RFC 0003's patterns, needs one record per account, and leaves the account key statement and every existing format as they are.
- Instead of: one agreement key per device, each published and signed, which the new device seals to one by one (B); an agreement key inside the account key statement, which changes every certificate, invitation and pairing answer (C); or requests left unsealed with only the answer sealed, which shows the relay the kind and whether a database links its own account (D).
- Topics: pairing, crypto, relay, accounts, rfc-0019
