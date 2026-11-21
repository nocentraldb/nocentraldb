# 2026-11-07 · NOC-6 · Every payload is padded to a multiple of 256 bytes before it is encrypted

- Decided by: Carsten (RFC 0002 decision D8)
- Why: it hides the exact length of short values, such as "yes" against "no", at a cost of 128 bytes per change on average; padding is a payload key readers ignore, so the rule can change later without a new format.
- Instead of: no padding, which hides nothing of a write's length; padding to the next power of two, which hides more but doubles the size of large changes such as rich-text pastes.
- Topics: rfc-0002, change-format, crypto, relay

The exact rule is in `docs/rfc/0003-keys-and-cryptography.md`, "Sealed payload".
