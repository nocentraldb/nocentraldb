# 2026-11-21 · NOC-412 · The Rust SDK has its own Date and Timestamp types, and a SystemTime rounds down to the millisecond

- Decided by: Carsten, 2026-11-21, on NOC-434 (answer 5a)
- Why: as with Reference, the public API does not follow another crate's major versions; rounding toward the past makes both SDKs store the same value for the same instant.
- Instead of: chrono, time or jiff types in the public API.
- Topics: rfc-0005, rfc-0009, sdk, rust, dates, public-names
