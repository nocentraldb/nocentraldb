# 2026-11-08 · NOC-7 · RFC 0003 defines the bytes keys sign and derive, each hash input under a context string of its own

- Decided by: Carsten (RFC 0003 decision 15)
- Why: every hash input starts with its own context string and has fixed-length fields or canonical CBOR, so no two inputs can be read as the same bytes; Ed25519 signs the 32-byte hash, which keeps every signed message one length and matches RFC 0007's relay challenge. The formats were proposed for RFC 0008 and accepted with it: the member certificate, the account sign-off of a shred, the key-chain subkey and the key envelope's `info`.
- Instead of: no other option was put to the owner.
- Topics: rfc-0003, crypto, keys, wire-format

The formats, as later decisions extended them, are in `docs/rfc/0003-keys-and-cryptography.md`, "Formats of what keys sign and derive". That a parent agent, not the account, signs a sub-agent's certificate is in 2026-11-08-NOC-23-rfc-0008-has-the-parent-agent-sign-a-sub-agent-s-certificate-as-rfc-0003-does.md.
