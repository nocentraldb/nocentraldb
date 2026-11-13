# 2026-11-13 · NOC-176 · The TypeScript sync status names its relay

- Decided by: Carsten, on #317 (pending tick); pm cleared it for the owner's decision
- Why: RFC 0009's one-page example shows `db.sync.current()` with `relay: "wss://relay.example.com"`, but its table of the report's values does not list it; the omission is a gap in the text. `SyncStatus.relay` is the relay `nc.open` named, and is absent without one. A one-line row for RFC 0009's table follows as a documentation PR once the owner ticks #317.
- Instead of: following the table and leaving `relay` out, which contradicts the RFC's own example.
- Topics: sdk-ts, sync, rfc-0009
