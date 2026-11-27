# 2026-11-21 · NOC-412 · A calendar day and a millisecond point in time in UTC are two basic value types

- Decided by: Carsten, 2026-11-21, on NOC-412
- Why: a birthday is the same day in every time zone, while an event happens at one instant that falls on different days around the world, so one type cannot serve both; both merge as basic values (the last writer wins, losing writes stay readable), so no new merge rule is needed. Apps that use NoCentralDB kept these dates as ISO text, which sorts and checks badly.
- Instead of: one point-in-time type with a day as midnight UTC, which shows the day before west of UTC; dates kept as text in `nc.string()` fields.
- Topics: rfc-0004, rfc-0005, field-types, dates

The names, the encoding, the index order and the SDK types are in RFC 0004 and RFC 0005, "Days and points in time".
