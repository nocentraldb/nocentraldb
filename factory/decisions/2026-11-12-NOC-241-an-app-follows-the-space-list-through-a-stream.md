# 2026-11-12 · NOC-241 · An app follows the space list through a stream

- Decided by: Carsten, 2026-11-12, on NOC-241 (A)
- Why: an app must learn that another device shared a space with this member, and that a space was left, removed or changed role, without polling. `db.spaces.subscribe()` (Rust: `Spaces::subscribe() -> impl Stream<Item = Vec<SpaceInfo>>`) yields the current list at once and then each new list when it changes, in the shape of the other reports.
- Instead of: a `spaceAdded` notice, which would cover only one of these changes; and polling `db.spaces.list()`.
- Topics: api, spaces, sync
