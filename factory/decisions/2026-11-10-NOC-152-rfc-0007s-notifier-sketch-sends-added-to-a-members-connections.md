# 2026-11-10 · NOC-152 · RFC 0007's notifier sketch sends `added` to a member's connections, and the cost note assumes three subscriptions per connection

- Decided by: coding agent (NOC-152)
- Why: the owner's answer on NOC-143 (option A) has the relay look up a member's open connections and post `added`, but RFC 0007's `Notifier` sketch only delivers by space. One method, `added(member, space)`, best-effort like `publish`, says where that delivery lives without a new wire detail; the built relay server has no `Notifier` trait yet, so nothing in code changes. The cost note puts the index on member id at about 5 USD a month for 1,000 agents, from the 1 M reconnects of the cost table and an assumed three subscriptions per connection, and keeps both totals as they are.
- Instead of: leaving the sketch without a way to reach a member's connections, which would make the AWS text the only place that says how `added` is delivered; or adding a row for connection writes to both cost tables, which count no connection writes today.
- Topics: relay, rfc-0007, aws
