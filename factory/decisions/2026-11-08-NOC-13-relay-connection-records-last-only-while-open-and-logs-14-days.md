# 2026-11-08 · NOC-13 · Relay connection records last only while open and logs 14 days by default; an operator may keep both longer

- Decided by: Carsten (RFC 0007 decisions R9 and R11, and Conflicts point 1)
- Why: a network address is personal data under the GDPR, so by default the server keeps no record of a connection once it closes; an operator may need records for abuse investigation and may keep them for a period it chooses, 30 days by default (R9). Logs hold ids, sizes, error codes, times and, in access logs, network addresses, never content or key material, and 14 days cost under 1 USD a month for a small team; the operator may set any longer period (R11). "What the relay sees" lists what the relay learns apart from what the server (AWS) sees, and names the AWS setting each item depends on (Conflicts point 1).
- Instead of: connection records always kept after the connection ends (R9); 90 days, or 1 day with no network addresses in any log (R11).
- Topics: relay, rfc-0007, aws, privacy

The full list stands in `docs/rfc/0007-relay.md`, "What the relay sees".
