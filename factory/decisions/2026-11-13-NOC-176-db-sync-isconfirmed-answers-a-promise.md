# 2026-11-13 · NOC-176 · `db.sync.isConfirmed` answers a promise in the TypeScript SDK

- Decided by: Carsten, on #317 (pending tick); pm cleared it for the owner's decision
- Why: RFC 0009 does not say whether `isConfirmed` is synchronous. In the browser the core runs in a worker, so a synchronous answer would need a cache of every confirmed change id on the page, which grows without bound. `db.sync.isConfirmed(change)` asks the core and answers `Promise<boolean>`; `db.sync.confirmed(change)` is a `Promise<void>`, as RFC 0009 says.
- Instead of: a synchronous `isConfirmed` over a page-side cache of confirmed ids.
- Topics: sdk-ts, sync
