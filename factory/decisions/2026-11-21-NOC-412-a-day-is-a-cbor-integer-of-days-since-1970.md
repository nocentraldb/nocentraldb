# 2026-11-21 · NOC-412 · A day travels as a CBOR integer of days since 1970-01-01, a point in time as one of milliseconds

- Decided by: Carsten, 2026-11-21, on NOC-434 (answer 2a)
- Why: plain CBOR in RFC 0002's profile with one encoding per value, three bytes for a day of this century, every integer in range is a valid day, and it sorts and subtracts as a number.
- Instead of: the text "YYYY-MM-DD", which every reader must parse and check; the integer 20261121, of which most values name no day; CBOR's date tags, which RFC 0002's profile forbids.
- Topics: rfc-0004, wire-format, cbor, dates
