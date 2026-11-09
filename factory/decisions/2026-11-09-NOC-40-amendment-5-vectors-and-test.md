# 2026-11-09 · NOC-40 · RFC 0002 Amendment 5's examples have their own vectors folder and tooling test

- Decided by: coding agent (NOC-40)
- Why: the examples M1 and M2 are written by `docs/rfc/vectors/0002/generate.py` with cbor2 6.2.1, as NOC-37 did for RFC 0004 (#52), and checked by `scripts/rfc-0002-model.test.mjs` with the shared profile reader; a separate test file keeps `scripts/rfc.test.mjs` to RFC 0002's earlier amendments and well under the file-size limit.
- Instead of: adding the checks to `scripts/rfc.test.mjs`, or checking the bytes outside the gate.
- Topics: rfc-0002, wire-format, testing, cbor
