# 2026-11-08 · NOC-37 · Field operations carry an argument map in key 5, numbers from one table, and are parked one by one

- Decided by: Carsten (RFC 0004 Amendment 1, decisions 8 to 12)
- Why: RFC 0002's key 5 already belongs to the field type, so the arguments sit there as a map with small integer keys (4 to 63 must be understood, 64 up may be ignored) and RFC 0002 stays as it is. One table numbers the built-in operations (0 create, 1 set, 2 add, 3 remove, 4 delete; 5 to 63 for later built-in types; 64 up for plugin types), so an operation sent to the wrong type is parked, not misread. Operation ids are `[change id, position]` in lists sorted by bytes, empty lists written as `[]`, so each operation has one encoding. The SDK writes a transaction's net effect per field, so there is no second form of id. An unknown or malformed operation is parked and reported on its own, and the rest of its change is folded: no new state, the same on every member, and nothing applied that the reader had to guess.
- Instead of: new keys 6 and 7 in RFC 0002's operation map, or a positional array; numbers per type; one byte string per id, or leaving empty lists out; bare positions naming operations of the same change; reading malformed arguments leniently, or rejecting the whole change.
- Topics: rfc-0004, wire-format, parking

The layout, the argument keys and the byte-exact examples are in `docs/rfc/0004-merge-rules-basic-types.md`, "How field operations travel in a change"; the parking cases in "Operations a member does not understand".
