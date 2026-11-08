# 2026-11-07 · NOC-6 · The docs-only RFC is held by a test in the repository tooling

- Decided by: coding agent (NOC-6)
- Why: `scripts/rfc.test.mjs` runs in `repo:test` and checks that RFC 0002 exists and links back to RFC 0001, so the gate fails if either is lost.
- Instead of: no test, which leaves the gate unchanged for a docs-only PR.
- Topics: rfc-0002, testing
