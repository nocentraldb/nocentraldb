# 2026-11-10 · NOC-153 · `added` lifts the Rust SDK's stop of a space, and `SyncStatus::error` drops the removal once no space stays stopped

- Decided by: coding agent (NOC-153), as the ticket asks for a space stopped after #141
- Why: NOC-158 stops syncing a space once the relay answers `not_member` or `removed_member`, keeps the stop in memory, and keeps the code in `SyncStatus::error`. A member re-added after a rotation (RFC 0008, M5) gets `added` for that space, so the SDK lifts its stop and pulls, pushes and subscribes to it as for a space named in `welcome`; `spaces().list()` shows the role its copy folds again. `SyncStatus::error` holds one code for all spaces, so it drops the removal only once no space stays stopped: while another space is still stopped, the code still describes it.
- Instead of: clearing the code at the first `added`, which would hide a removal from another space that is still stopped; or keeping the code after every stop is lifted, which would report a removal that no longer holds.
- What this does not do: `added` for a space the member already follows changes nothing; RFC 0009's per-space status, which would carry a removal per space, is still NOC-120.
- Topics: sdk-rust, rfc-0007, errors
