# 2026-11-14 · NOC-317 · The TypeScript oldest unconfirmed time is milliseconds since the epoch

- Decided by: Carsten, 2026-11-14, on NOC-317 (A)
- Why: RFC 0006 says `db.durability`'s `oldestUnconfirmedAt` is "the local time" of the oldest unconfirmed change, or `null`. In TypeScript it is a number, milliseconds since the epoch, as the space status already gives ages in milliseconds (#317) and the core keeps `written_at`; RFC 0006 says so.
- Instead of: a `Date` (option B).
- Topics: sdk-ts, durability, rfc-0006
