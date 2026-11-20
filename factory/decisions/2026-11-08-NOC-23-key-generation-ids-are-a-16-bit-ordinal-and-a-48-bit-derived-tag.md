# 2026-11-08 · NOC-23 · Key generation ids are a 16-bit ordinal and a 48-bit tag derived from space, author and previous generations

- Decided by: Carsten (RFC 0008 decision M6)
- Why: two islands can rotate at once, and two keys under one generation number make every change under it ambiguous. A tag derived from the space id, the rotation's author and its previous generation ids makes two honest rotations differ, keeps RFC 0002's 64-bit header key 4, and leaves 65,535 rotations per space, enough for v1. An admin that searches about 2⁴⁸ member keys can make one rotation collide, which blocks it and exposes no key.
- Instead of: generation numbers only, with the lower key chain entry hash winning (RFC 0003's sketch); a 32-byte generation id in the header, which changes RFC 0002's format.
- Topics: rfc-0008, keys, rotation, wire-format

The layout is in `docs/rfc/0008-membership.md`, "Generation ids". What happens to two rotations with one id (M23) and to changes under a rejected rotation (M17) is in `2026-11-10-NOC-71-the-generation-graph-holds-every-key-chain-entry-valid-when.md`.
