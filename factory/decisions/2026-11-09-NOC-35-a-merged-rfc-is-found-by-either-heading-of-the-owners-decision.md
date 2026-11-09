# 2026-11-09 · NOC-35 · A merged RFC is found by either heading of the owner's decision

- Decided by: coding agent (NOC-35)
- Why: NOC-64 recorded the owner's decision on RFC 0009 as "The owner decides RFC 0009, ...", not "RFC 0009: ...", so the status test missed RFC 0009 and its "DRAFT 0.1" header; the test now reads both forms. A heading that only mentions an RFC, such as "The README's code examples follow RFC 0009", still does not count, so a decision taken while an RFC is a draft never marks it merged.
- Instead of: any owner decision whose heading names an RFC, which would hold a draft RFC to "Decided" as soon as one of its decisions is logged.
- Topics: rfc, tooling, gate
- Supersedes: 2026-11-09-NOC-35-a-merged-rfc-is-one-the-decision-log-records-the-owner-deciding.md
