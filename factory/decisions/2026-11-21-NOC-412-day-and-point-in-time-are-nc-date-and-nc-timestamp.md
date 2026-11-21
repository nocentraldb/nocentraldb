# 2026-11-21 · NOC-412 · A day is nc.date() and ncdb/date, a point in time nc.timestamp() and ncdb/timestamp, in Rust Date and Timestamp

- Decided by: Carsten, 2026-11-21, on NOC-434 (answer 1a)
- Why: the names SQL and jiff use, which most developers know; one name per type in the schema, the lock and the Rust SDK.
- Instead of: Temporal's `nc.plainDate()` and `nc.instant()`, exact but less familiar; `nc.day()` and `nc.time()`.
- Topics: rfc-0005, field-types, dates, schema, public-names
