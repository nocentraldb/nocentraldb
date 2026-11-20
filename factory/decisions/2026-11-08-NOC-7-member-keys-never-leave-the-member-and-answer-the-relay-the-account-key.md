# 2026-11-08 · NOC-7 · Member keys never leave the member and answer the relay's challenge; the account key is on every device

- Decided by: Carsten (RFC 0003 decisions 4, 8 and 13)
- Why: each member creates its Ed25519 and X25519 keys on itself and keeps them in the platform keystore (or a key file a headless agent names). The same signing key signs the relay's challenge under its own context string, because that string keeps the two uses apart, the relay must know the signing key anyway to verify changes, and a third key would add a certificate and a rotation without protecting more. The account key lives on every device of the account, plus a recovery code the person keeps, as the simplest for people to use in v1.
- Instead of: a third key per member only for relay authentication, certified by the account key; the account key only on designated admin devices (may come later as an option for organisations), or derived from a passkey.
- Topics: rfc-0003, keys, accounts, relay, recovery

The rules are in `docs/rfc/0003-keys-and-cryptography.md`, "The member key" and "The account key". Later decisions made the recovery code derive a recovery key that is the account's identity (NOC-191, NOC-285).
