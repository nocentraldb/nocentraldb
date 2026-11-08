# 2026-11-07 · NOC-13 · The relay's checks live in one core; backends only store

- Decided by: coding agent (NOC-13)
- Why: RFC 0007 puts every signature, membership and frame check in one relay crate, so SQLite, Postgres and DynamoDB with S3 cannot drift apart on a security check and stay interchangeable behind one storage interface and one conformance suite.
- Instead of: each backend implementing the checks itself.
- Topics: relay, rfc-0007, security
