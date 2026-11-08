# 2026-11-07 · NOC-23 · RFC 0008 lets irreversible effects of a shred wait until it is stable

- Decided by: coding agent (NOC-23), for the owner's ruling as decision M20
- Why: a concurrent revocation can still reject an accepted shred; destroying the key, acknowledging it and deleting the relay's record cannot be undone, so they wait until every admin that could revoke has seen the shred.
- Instead of: making accepted control changes final, which no member can know without the others.
- Topics: rfc-0008, membership, erasure, keys
