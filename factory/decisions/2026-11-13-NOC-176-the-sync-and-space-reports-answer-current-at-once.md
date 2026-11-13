# 2026-11-13 · NOC-176 · `db.sync.current()` and `space.status.current()` answer at once, from the core's events

- Decided by: Carsten, on #317 (pending tick); pm cleared it for the owner's decision
- Why: RFC 0009's examples read both reports synchronously. The core sends the first sync status with its answer to `open`, and a space's status with `space.status` when a handle is made; from then on its `sync` and `spaceStatus` events keep them current, so `current()` returns the newest without asking the worker.
- Instead of: `Promise<SyncStatus>` and `Promise<SpaceStatus>`, which differ from RFC 0009's examples and make every read of a report wait for the worker.
- Topics: sdk-ts, sync, status
