# 2026-11-09 · NOC-64 · Queries use declared index handles, skip deleted objects and can fix one space; subscriptions pass whole pages

- Decided by: Carsten, 2026-11-09, in chat with pm, on #78 (RFC 0009 decisions S7 to S10)
- Why: A query is a chain on the model's index handle, `model.tasks.indexes.byStatusPriority.equal("open").take(50)`, passed to `db.find`: RFC 0001's chain for views, with equality values checked by TypeScript (S7). An index option `scope: "space"` puts the space id first and `space.find` fixes it, because "the open tasks of this project" is the first screen of most apps (S8). Queries skip deleted objects and the index does not hold them, while `space.get` still returns them with `$deleted: true` so a deletion can be undone (S9). `db.subscribe` and `space.subscribe` pass the whole fresh page or object, which is what RFC 0006's notification is for, and `db.changes.subscribe` passes the ids each fold touched (S10).
- Instead of: an options object, `db.find(model.tasks, { index, equal, limit })` (S7 B); no query per space with the app filtering in memory, or every index per space with `db.find` merging, which reads rows it throws away or loses the page across spaces (S8 B, C); deleted objects returned flagged (S9 B); only a stream of changes with the app re-reading (S10 B); a `where` clause on any field, which needs a scan.
- Topics: rfc-0009, sdk, query, indexes, subscribe

Details: `docs/rfc/0009-sdk-api.md`, "Queries and indexes" and "Subscribe".
