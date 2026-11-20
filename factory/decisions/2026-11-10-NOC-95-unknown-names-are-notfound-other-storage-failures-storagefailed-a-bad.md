# 2026-11-10 · NOC-95 · Unknown names are notFound, other storage failures storageFailed, a bad query invalidValue, a bad lock modelRefused

- Decided by: Carsten, 2026-11-10, on NOC-95 (RFC 0009 decision S22, option B)
- Why: An unknown collection, field or index is `notFound`; a full disk is `storageFull` and any other storage failure the new `storageFailed`, because `storageFull` tells the app to free space, which does not help when a file is damaged; a wrong number of equality values, a bad page size or an index of the other scope is `invalidValue`; a lock that cannot be read or is invalid is `modelRefused`. It costs one more code.
- Instead of: every storage failure as `storageFull` (S22 A).
- Topics: rfc-0009, errors, sdk-rust
