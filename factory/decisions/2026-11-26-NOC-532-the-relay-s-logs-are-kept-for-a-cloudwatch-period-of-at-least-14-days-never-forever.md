# 2026-11-26 · NOC-532 · The relay's logs are kept for a CloudWatch Logs period of at least 14 days, never forever

- Decided by: coding agent (NOC-532)
- Why: RFC 0007 keeps logs "14 days by default, and any longer period the operator sets". `logRetentionDays` takes 14 or a longer period CloudWatch Logs offers, and the construct refuses anything else when it synthesizes. Keeping logs forever is no period, and a data sheet could not state when the logs go.
- Instead of: accepting CloudWatch's "never expire"; accepting shorter periods, which RFC 0007 does not offer (it did not take 1 day).
- Topics: relay, aws, cdk, privacy
