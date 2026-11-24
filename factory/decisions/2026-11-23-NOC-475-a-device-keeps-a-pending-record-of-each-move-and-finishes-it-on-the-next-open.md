# 2026-11-23 · NOC-475 · A device keeps a pending record of each move beside the view, and the next open finishes or drops it

- Decided by: coding agent, NOC-475
- Why: RFC 0016 has the SDK write the old space's move "when it opens again, from a pending record it keeps on the device". The record is written before the new space's change and cleared after the old space's move, in a table `pending_moves` that, like `ad_hoc_copies`, no fold computes and nothing syncs. On open, a record whose object the new space holds and the old one holds live gets its move; any other is dropped, since either the move was written or the new space never got the object.
- Instead of: writing the record in the same SQLite transaction as the new space's change, which would need `write_change` to take a hook, for no case the open's check does not already cover.
- Topics: rfc-0016, move, sdk-rust, view, recovery
