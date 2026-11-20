# 2026-11-10 · NOC-63 · A reference is the built-in type ncdb/ref, written by set, its value [space id, object id] as two byte strings

- Decided by: Carsten, 2026-11-10, on PR #92 (RFC 0004 decisions 15 and 16)
- Why: A new built-in type `ncdb/ref`, rule 1, written by set (1) with set's arguments and no other operation, keeps the one table of operation numbers, reuses basic values' merge rule without a new proof, and makes an old client park its operations as an unknown type instead of misreading them. The value is a CBOR array of two 16-byte byte strings, plain CBOR with one encoding, the form operation ids already use; 35 bytes.
- Instead of: `ncdb/link` with a new rule version whose value may name a space, which changes a link's meaning under its own name; a new operation number "set a reference", which adds nothing the type id does not say (decision 15); one 32-byte byte string, a hand-written layout inside CBOR; a map `{1: space id, 2: object id}`, larger with nothing more to carry (decision 16).
- Topics: rfc-0004, references, wire-format, field-types
