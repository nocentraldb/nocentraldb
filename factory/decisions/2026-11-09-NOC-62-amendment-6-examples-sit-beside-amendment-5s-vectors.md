# 2026-11-09 · NOC-62 · Amendment 6's examples N1 and N2 are payloads, written by the same generator as M1 and M2

- Decided by: coding agent (NOC-62)
- Why: the ticket asks for a value at level 32 and at level 33 from the same independent generator. N1 and N2 are Amendment 5's payload M2 with an operation value of nested one-item arrays, 27 and 28 levels deep, so the deepest item sits at level 32 and 33 counted from the frame, without a signature or ciphertext to fake. `docs/rfc/vectors/0002/generate.py` writes them beside M1 and M2; `scripts/rfc-0002-model.test.mjs` now counts only the M vectors in that folder, and `crates/ncdb-core/tests/depth.rs` decodes N1 and N2 with the core. The ticket's "field types keep 28" counts four levels above an operation's value; RFC 0002's own tree has five (frame, header, payload, operations, operation), so key 5's value may hold 27, and RFC 0004 Amendment 1's arguments map one fewer.
- Instead of: a bare value at depth 1, which shows the counting less clearly, or a full signed frame, which needs keys the generator does not have.
- Topics: rfc-0002, cbor, testing
