# 2026-11-07 · NOC-7 · RFC 0003's tooling test checks the RFC's shape, not its design

- Decided by: coding agent (NOC-7)
- Why: a test can prove the file exists, links to RFC 0001, resolves its links and has a section per acceptance criterion; whether the cryptography is right is the owner's review of the PR.
- Instead of: no test at all, or a test that pins the RFC's recommendations, which would have to change with every owner decision.
- Topics: rfc-0003, testing
