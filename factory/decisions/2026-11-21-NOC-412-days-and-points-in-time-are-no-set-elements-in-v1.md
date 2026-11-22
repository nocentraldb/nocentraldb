# 2026-11-21 · NOC-412 · Days and points in time are no set elements in v1

- Decided by: Carsten, 2026-11-21, on NOC-434 (answer 6a)
- Why: a set orders its elements by their canonical CBOR bytes, where every positive integer comes before every negative one, so days before 1970 would read out of order; the fields asked for hold one date each.
- Instead of: allowing nc.set(nc.date()) now. A set of dates can come later as a new field function.
- Topics: rfc-0005, field-types, dates, sets
