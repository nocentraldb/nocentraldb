# 2026-11-27 · NOC-552 · An edge is unique per pair: its id is SHA-256 of nocentraldb/edge, the collection name and both ends, cut to 16 bytes

- Decided by: Carsten, 2026-11-27, on RFC 0018's open questions, through pm (NOC-552)
- Why: Creating an edge again changes nothing, and devices that are offline never make two edges for one pair; the account space derives its record ids the same way.
- Instead of: Uniqueness only on request, or random ids with duplicates left to the app.
- Topics: rfc-0018, edges, ids, format

`docs/rfc/0018-model-api-for-placement.md` states it (NOC-551).
