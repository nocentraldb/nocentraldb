# 2026-11-08 · NOC-37 · RFC 0004 Amendment 1: every recommendation accepted

- Decided by: Carsten
- Why: the owner accepted decisions 8 to 14 of RFC 0004 Amendment 1 as recommended: field operations carry their arguments as a map in RFC 0002's key 5; one table of operation numbers (0 create, 1 set, 2 add, 3 remove, 4 delete; 5 to 63 for later built-in types, 64 up for plugins); operation ids as `[change id, position]` in sorted lists, empty lists written as `[]`; the SDK writes a transaction's net effect per field; unknown and malformed operations are parked one by one; a new operation is a new rule version or a new field type; an object's deletion is operation 4 with set's arguments.
- Instead of: the alternatives listed for each decision in `docs/rfc/0004-merge-rules-basic-types.md`, Amendment 1.
- Topics: rfc-0004, wire-format
