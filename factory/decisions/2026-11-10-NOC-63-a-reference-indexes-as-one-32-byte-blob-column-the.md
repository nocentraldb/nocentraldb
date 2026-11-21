# 2026-11-10 · NOC-63 · A reference indexes as one 32-byte BLOB column, the space id followed by the object id

- Decided by: Carsten, 2026-11-10, on PR #92 (RFC 0005 decision D19)
- Why: The index holds the reference as stored, whatever its state, so it is the same on every member of the space, and "all orders of customer X" is one equality lookup in the index shape RFC 0006 already has; an unset reference and a value that does not fit index as unset.
- Instead of: two columns, which no other type makes of one field; the object id only, which a hostile member could use to make an order match a customer of another space.
- Topics: rfc-0005, rfc-0006, references, indexes
