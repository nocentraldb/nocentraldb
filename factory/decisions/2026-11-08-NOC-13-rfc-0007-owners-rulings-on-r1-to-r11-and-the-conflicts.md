# 2026-11-08 · NOC-13 · RFC 0007: the owner's rulings on R1 to R11 and the conflicts

- Decided by: Carsten
- Why: R1, R3, R4, R5, R6, R8 and R10 take the recommendation (A). R2 is A, and a developer's own adapter may put its own authentication, such as Cognito, in front of the connection; our AWS backend does not. R7 was already decided by RFC 0008, M3. R9 is configurable: by default a connection record lives only while the connection is open (A), and an operator may keep records for a period it chooses, 30 days by default (B). R11 keeps logs 14 days by default, and the operator may set any longer period. Conflicts point 1: "What the relay sees" separates what the relay learns from what the server (AWS) sees, and names the AWS setting each item depends on. Points 2 to 4 are accepted; the changes they need in other documents are NOC-47.
- Instead of: the alternatives listed for each decision in `docs/rfc/0007-relay.md`, and changing RFC 0002, the use cases and the FAQ in this pull request.
- Topics: relay, rfc-0007, aws, privacy, cost
