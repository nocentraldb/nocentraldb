# 2026-11-07 · NOC-22 · RFC 0002's two gaps become an errata ticket, not fixes in the later RFCs

- Decided by: pm
- Why: RFC 0002 (merged) says a change is atomic and also parks single operations, and has no log state for a change that failed a check; RFC 0003, 0005, 0006 and 0007 each lean on one reading. An amendment to RFC 0002 settles it once, and the owner signs it off as a format change.
- Instead of: letting each later RFC define its own reading, which would leave four RFCs to reconcile later.
- Topics: rfc-0002, rfc, planning
