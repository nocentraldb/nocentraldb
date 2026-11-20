# 2026-11-09 · NOC-64 · The Rust SDK has the same calls in snake case and reads and writes fields by name with Value

- Decided by: Carsten, 2026-11-09, in chat with pm, on #78 (RFC 0009 decision S18)
- Why: RFC 0005 leaves code generation for later; reading fields by name works for every lock today, and generated structs can be added without changing it.
- Instead of: typed structs generated from the lock in v1 (S18 B).
- Topics: rfc-0009, sdk-rust, api
