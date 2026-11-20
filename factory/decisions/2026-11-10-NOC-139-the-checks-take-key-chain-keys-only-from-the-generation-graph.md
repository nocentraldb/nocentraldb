# 2026-11-10 · NOC-139 · The checks take key chain keys only from entries the generation graph holds, and never over a held key

- Decided by: coding agent (NOC-139)
- Why: RFC 0003 and RFC 0008 leave open, inside the core, which key chain entries a member opens and what wins when two give a key for one generation. `SealChecks` opens the previous keys only of entries the generation graph holds (valid when written), so a member that held a key without the admin role cannot plant keys; a key from the chain fills only a generation the copy holds no key for, an envelope to the member replaces it, and entries are taken in change id order, so a copy keeps the same key each time.
- Instead of: opening every key chain entry that decrypts, which would let a writer that once held a generation's key write a rotation that hands members a wrong key for an older generation.
- Topics: keys, membership, rfc-0003, rfc-0008
