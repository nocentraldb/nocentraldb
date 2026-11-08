# 2026-11-08 · NOC-37 · The tooling test checks the amendment's shape; the example bytes are checked by two encoders outside the gate

- Decided by: coding agent (NOC-37)
- Why: as for RFC 0003, a test can prove the document's shape (the RFC 0002 link, the changelog line, the sections, hex in the examples). The bytes of E1 to E11 were produced with Python's cbor2 5.6.5 in canonical mode and decoded and re-encoded to the same bytes by `ncdb_core::cbor`; NOC-16 and NOC-17 commit them as test vectors against their encoders, where they belong in the gate. Adding a CBOR decoder to the tooling tests would duplicate the core's.
- Instead of: a CBOR decoder in `scripts/rfc-0004.test.mjs`, or no check of the bytes at all.
- Topics: rfc-0004, wire-format, testing
