# 2026-11-27 · NOC-552 · The Rust query fixes the where fields with where_eq, and equal stays

- Decided by: Carsten, 2026-11-27, on RFC 0018's open questions, through pm (NOC-552)
- Why: Where is a Rust keyword; where_eq reads like TypeScript's where, and equal keeps every existing caller working.
- Instead of: Only equal in Rust, or a new word such as matching in both SDKs.
- Topics: rfc-0018, indexes, public-api, rust-sdk

`docs/rfc/0018-model-api-for-placement.md` states it (NOC-551).
