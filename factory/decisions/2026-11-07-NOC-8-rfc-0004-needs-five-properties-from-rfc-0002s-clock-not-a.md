# 2026-11-07 · NOC-8 · RFC 0004 needs five properties from RFC 0002's clock, not a specific clock

- Decided by: coding agent (NOC-8)
- Why: RFC 0002 runs in parallel; RFC 0004 lists what it needs (unique change ids, a total stamp order computed from the change alone, causality, wall-clock component wanted, one canonical encoding per value) so the two RFCs can be reconciled before merge.
- Instead of: assuming a hybrid logical clock and its encoding in RFC 0004.
- Topics: rfc-0004, rfc-0002, clock
