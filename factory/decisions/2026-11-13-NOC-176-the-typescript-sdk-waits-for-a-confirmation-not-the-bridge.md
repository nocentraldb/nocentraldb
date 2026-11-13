# 2026-11-13 · NOC-176 · The TypeScript SDK waits for a confirmation, not the bridge

- Decided by: pm, NOC-176, on the coding agent's proposal; named in #317 for the owner
- Why: the worker answers calls one at a time, in order, so a bridge call that waited for the relay to confirm a change would hold back every call after it, possibly forever, since a confirmation may never come. The bridge answers `sync.isConfirmed` at once instead, and `db.sync.confirmed(change)` waits in TypeScript: it registers for the next `sync` event before asking, so no confirmation slips between the answer and the wait, and asks again after each one. A confirmation lowers `unconfirmed`, so it always arrives as a `sync` event.
- Instead of: a `sync.confirmed` call in the bridge, which blocks the worker's queue while it waits.
- Topics: sdk-ts, sync, bridge
