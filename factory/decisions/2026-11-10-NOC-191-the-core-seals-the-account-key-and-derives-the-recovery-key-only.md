# 2026-11-10 · NOC-191 · The core seals the pairing answer's account key and derives the recovery key, and nothing of the flows yet

- Decided by: coding agent (NOC-191), inside the core
- Why: the ticket asks for RFC 0003's bytes, not RFC 0010's flows. `keys::RecoveryKey::from_code` derives the recovery key from the code's 128 bits (HKDF-SHA256, no salt, info `"nocentraldb/recovery-key"`, 32 bytes as the Ed25519 seed) and shows only its public half; `pairing::seal_account_key` and `open_account_key` seal the account key's 32-byte Ed25519 secret with the key envelope's HPKE construction under the info `"nocentraldb/account-key"` alone, and share one seal and open with `envelope`. The pairing answer's container, with its certificate and statement, the recovery code's words and the SDK names come with RFC 0010's build tickets, which define them.
- Instead of: a byte layout for the whole pairing answer now, which RFC 0003 does not give; a second HPKE implementation for the pairing, which would let the two drift.
- Topics: crypto, rfc-0003, rfc-0010
