# 2026-11-10 · NOC-99 · nc.ref is a field type without `personal`, and its fragment is breaking because the API report says so

- Decided by: coding agent (NOC-99)
- Why: RFC 0005's D22 gives a reference `.describe` and `.deprecated` only. The SDK types it as its own interface `Ref`, which has no `personal`, so calling it is a type error and the object has no such method at run time. A collection's fields are now typed by `Value` (what a field reads as, and whether it is deprecated), which both `Field` and `Ref` extend. No schema that compiled before stops compiling, but `scripts/release/api-report.mjs` compares lines and reports the reworked internal lines of `Field`, `Fields` and `nc.Infer` as breaking, and the `changes` check then requires a breaking fragment, which the PR carries with a migration that says no code needs to change.
- Instead of: a `personal` method typed to take `never` on a reference, which keeps the old lines but leaves a method D22 says a reference does not have.
- Topics: sdk, api, schema, rfc-0005, references, api-report
