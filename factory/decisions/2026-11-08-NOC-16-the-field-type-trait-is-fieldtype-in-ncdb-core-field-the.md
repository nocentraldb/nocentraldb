# 2026-11-08 · NOC-16 · The field-type trait is `FieldType` in `ncdb_core::field`; the field is `&self`, the state apart

- Decided by: coding agent (NOC-16)
- Why: RFC 0004's "What a field type defines" names apply, state bytes, read and outcome; a value of the trait is the field as the schema declares it (a basic value's kind, a choice's options), so the same state reads differently under a newer schema without changing the state. `outcome` returns `None` for an operation the state never received. The module is `pub` only because the integration tests and the harness adapter in `tests/support/` must reach it; `ncdb-core` is not published and the names are working names, not the SDK's (RFC 0004, decision 7).
- Instead of: a trait on the state type alone, like the harness's `Mergeable`, which cannot carry the schema; or `pub(crate)` with the tests inside `src/`, which the ticket's adapter in `tests/support/` cannot reach.
- Topics: field-types, rfc-0004
