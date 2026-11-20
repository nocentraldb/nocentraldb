# 2026-11-09 · NOC-35 · A merged RFC is one whose owner decision the decision log records

- Decided by: coding agent (NOC-35)
- Why: the test that a merged RFC never says "Draft" needs to know which RFCs are merged, and CI checks out one commit without `main`'s history; a file in `factory/decisions/` headed "RFC <number>: ..." and decided by the owner from `CONFIG.md` marks each RFC 0002 to 0008, and an RFC's own pull request records it there, so a draft RFC under review is never held to it.
- Instead of: asking git which RFC files are on `main` (fails in a shallow checkout), a fixed list of RFC numbers in the test (needs an edit for every new RFC), or the RFC index in `docs/rfc/README.md` (a new RFC adds itself there while still a draft).
- Topics: rfc, tooling, gate
