# 2026-11-08 · NOC-37 · The gate checks the bytes of RFC 0004's examples, with a reader of RFC 0002's profile in the tooling tests

- Decided by: pm, in its review of PR #52
- Why: NOC-16 and NOC-17 build their encoders from E1 to E11, so a wrong byte would become a wrong wire format. `docs/rfc/vectors/0004/generate.py` writes the vectors with cbor2 6.2.1, the version NOC-14 pinned; `scripts/rfc-0004-examples.test.mjs` checks that each example in the RFC decodes under the profile, re-encodes to the same bytes and equals its vector. The reader in `scripts/lib/cbor-profile.mjs` is written apart from the core and from cbor2, needs no new dependency, and its fixtures prove it refuses each kind of non-canonical input. Reverses "The tooling test checks the amendment's shape; the example bytes are checked by two encoders outside the gate".
- Instead of: leaving the bytes to NOC-16 and NOC-17's test vectors, or a pinned npm CBOR library, whose deterministic mode is not RFC 0002's profile and would need the same re-encoding check.
- Topics: rfc-0004, wire-format, testing, cbor
- Supersedes: 2026-11-08-NOC-37-example-bytes-checked-outside-the-gate.md
