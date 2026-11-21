# 2026-11-21 · NOC-412 · Days and points in time range over the years 1 to 9999

- Decided by: Carsten, 2026-11-21, on NOC-434 (answer 3a)
- Why: every value prints as RFC 3339 with a four-digit year, and a JavaScript Date and number hold every value exactly; a value outside the range reads as one that does not fit, and the SDKs refuse to write it.
- Instead of: the full 64-bit range, which a JavaScript Date cannot hold and which prints in forms most parsers refuse.
- Topics: rfc-0004, dates, field-types
