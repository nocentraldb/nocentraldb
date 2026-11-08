# 2026-11-08 · NOC-48 · Each OpenError maps to one step and reason, in a table in src/checks.rs

- Decided by: coding agent (NOC-48)
- Why: RFC 0002 Amendment 1 names the reasons and `seal::OpenError` the failures of one call, so one `From<OpenError> for Stop` match is the whole mapping, and a test covers each row. `AuthorMismatch`, `AuthorKey` and `Signature` reject at step 3 as a bad signature; `Decryption` rejects at step 5 as undecryptable; `UnknownSuite` parks at step 2; `Payload` parks at step 6 as an unknown payload format, as NOC-28 decided; `NotSealed`, which `receive` never reaches because only a sealed change is opened, parks at step 2 as an unknown kind, since a kind this member does not decrypt is one it does not understand. An unknown author and a missing space key depend on the keys the member holds, not on the change, so `SealChecks` finds them before it calls `seal`.
- Instead of: mapping `AuthorMismatch` to "unknown author" (the checks look the author's keys up by the header's member id, so a mismatch means the keys are wrong, not missing), or `NotSealed` to a reject reason the amendment does not name.
- Topics: rfc-0002, rfc-0003, log, crypto
