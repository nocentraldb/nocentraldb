# 2026-11-07 · NOC-6 · An object id is 128 random bits, and a change id is the SHA-256 hash of the signed header bytes

- Decided by: Carsten (RFC 0002 decisions D2 and D3)
- Why: a hashed change id is unique without a coordinator, every party computes it on its own, and a backup restore cannot make two changes share it. A random object id from a secure generator leaks nothing about when an object was made into the links, URLs and exports it travels into.
- Instead of: for change ids, the author member id plus author sequence, or a random id; for object ids, UUIDv7 (48 bits of creation time and 74 random bits).
- Topics: rfc-0002, ids, change-format, crypto

`docs/rfc/0002-change-and-log-format.md`, "Ids without a coordinator", gives the exact hash input, operation ids and the options not taken. D3 also covered space ids; those are no longer random but derived from the creator's member id and a nonce in the genesis (`docs/rfc/0008-membership.md`, "What it names"), and an account-space record's object id derives from what it records (NOC-266).
