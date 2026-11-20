# 2026-11-08 · NOC-7 · members.add takes an invitation, and every call that changes who can read what returns a notice

- Decided by: Carsten (RFC 0003 decision 12 and reading 4)
- Why: passing an invitation or a member id instead of a public key keeps developers away from key material altogether, and a returned notice keeps a change of who can read what from being silent (GENESIS.md, "What it must never do"). Use case 3 follows it.
- Instead of: public keys passed to `members.add`, as use case 3 had it (`agentPublicKey`).
- Topics: rfc-0003, keys, api, notices, membership

The calls (`members.add`, `members.remove`, `workOrders.erase`, invitations) are in `docs/rfc/0003-keys-and-cryptography.md`, "How the SDK hides keys"; their public names are RFC 0009's.
