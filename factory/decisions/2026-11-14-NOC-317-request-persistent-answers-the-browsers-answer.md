# 2026-11-14 · NOC-317 · requestPersistent() answers the browser's answer

- Decided by: Carsten, 2026-11-14, on NOC-317 (A)
- Why: RFC 0006 says `db.durability.requestPersistent()` "resolves when the browser answers", but not with what. It resolves with the browser's answer, `Promise<boolean>`, and the new storage is also reported through `subscribe`; where the browser has no `persist()`, it answers `false`. RFC 0006 says so.
- Instead of: `Promise<void>` (option B), with the app reading the result through `current()` or `subscribe`.
- Topics: sdk-ts, durability, rfc-0006
