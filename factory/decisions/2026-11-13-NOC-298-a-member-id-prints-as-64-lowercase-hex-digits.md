# 2026-11-13 · NOC-298 · A member id prints and parses as 64 lowercase hex digits, in both SDKs

- Decided by: Carsten, 2026-11-13, on NOC-298 (option A)
- Why: the TypeScript SDK needs a string for a member in `members.list()[].id`, which `members.remove` and `members.setRole` take, and in a provisional erasure's `performer`. A member id is a hash of the member's two public keys, which every invitation carries, so its hex form shows no secret. As 64 lowercase hex digits it reads like the other ids, and an app can store it and use it on another device. Rust's `MemberId` gains `Display` and `FromStr`, and its `Debug` shows the same digits.
- Instead of: an opaque handle that only the database that made it accepts (B), with which a second device could not name a member to remove; or the member's fingerprint digits (C), which are longer and which RFC 0010 already uses for accounts.
- Topics: sdk-rust, sdk-ts, membership, ids
