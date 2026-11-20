# 2026-11-08 · NOC-13 · The AWS relay runs on API Gateway WebSockets and Lambda in eu-central-1, with 35 days of point-in-time recovery

- Decided by: Carsten (RFC 0007 decisions R3, R8 and R10)
- Why: API Gateway and Lambda, with live delivery from the appending Lambda, cost about 10 USD a month for a small team and nothing while idle; the relay moves to a Fargate server when API Gateway messages pass about 300 USD a month, and a budget alarm at 50 USD a month warns well before (R3). Frankfurt is closer to the first users, and the metadata the relay keeps is personal data under the GDPR, at about 10 to 20 percent more cost (R8). Point-in-time recovery for 35 days, with S3 versioning off and a new relay epoch on every restore, brings back the membership table and the person-key store before any member reconnects; a shredded person-key record stays in the backups for up to 35 days, wrapped (R10).
- Instead of: live delivery through DynamoDB Streams and a second Lambda, or a WebSocket server on ECS Fargate, at least 45 USD a month (R3); us-east-1, N. Virginia (R8); no backups, with members repopulating a lost relay (R10).
- Topics: relay, rfc-0007, aws, cost, privacy

The design stands in `docs/rfc/0007-relay.md`, "The AWS backend" (with "Delivery to subscribers" and "Restores and the relay epoch") and "What it costs".
