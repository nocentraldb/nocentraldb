# 2026-11-08 · NOC-17 · Links reuse basic values and sets; resolution is a read against the copy's objects

- Decided by: coding agent (NOC-17)
- Why: RFC 0004 gives links no merge rules of their own, so `Link` merges by the code of basic values and `Links` by the code of sets, over object ids (16 bytes) instead of a `Kind`. The crate-internal `basic::Values` says which values a field allows; `Kind` gets no new variant, because adding one to that public enum is a breaking change by cargo-semver-checks (CI, PR #53). Both implement `FieldType`, and the harness runs them as themselves. The status is not state, so it is not `FieldType::read`: `resolve` takes the copy's objects (`link::Objects`). The writing side takes the target with its space and refuses another space (`OtherSpace`); a removal takes any object id, so a stray link can be removed.
- Instead of: link field types with their own state, or a status stored in the state, which RFC 0004 rules out.
- Topics: field-types, links, rfc-0004
