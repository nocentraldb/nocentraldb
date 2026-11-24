# 2026-11-23 · NOC-475 · The Rust SDK reads a later edit as an `Edit` that `carry_over` writes again, and an edit it cannot write is `Edit::Other`

- Decided by: coding agent, NOC-475
- Why: RFC 0016's `space.moves.laterEdits()` gives `[{ id, edits }]` and `carryOver(edits)` writes chosen ones into the new space as one write, but leaves the shape of an edit open. In Rust an edit is the write it makes (`Set`, `Add`, `Remove`, `Delete`, `Restore`), named by its change and position; `carry_over` takes the caller's edits only as names and writes them as the old space holds them, so a changed copy cannot write anything else. A personal value, a plugin's operation or a create reads as `Edit::Other` and is refused with `NotAllowed`; edits of objects moved to different spaces are refused with `InvalidValue`, one write per space.
- Instead of: handing the app the raw operation, which would make it decode RFC 0004's operations itself.
- Topics: rfc-0016, move, later-edits, sdk-rust
