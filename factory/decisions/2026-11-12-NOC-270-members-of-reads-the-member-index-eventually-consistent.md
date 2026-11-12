# 2026-11-12 · NOC-270 · `members_of` on AWS reads the index on member id, which DynamoDB keeps eventually consistent

- Decided by: coding agent (NOC-270)
- Why: RFC 0007 answers "which spaces is this member in" with a global secondary index on `members`, and a global secondary index cannot be read consistently, so a membership an append just wrote may be missing for a moment; the relay uses the answer for `welcome`, where a member that misses a space learns of it from `added` or the next connection.
- Instead of: a second item per membership under the member id, written in the append's transaction, which reads consistently but adds an item per effect and a table RFC 0007 does not have.
- Topics: relay, aws, rfc-0007, consistency
