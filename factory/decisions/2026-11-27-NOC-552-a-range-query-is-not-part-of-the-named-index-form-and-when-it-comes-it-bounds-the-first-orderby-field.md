# 2026-11-27 · NOC-552 · A range query is not part of the named index form, and when it comes it bounds the first orderBy field

- Decided by: Carsten, 2026-11-27, on RFC 0018's open questions, through pm (NOC-552)
- Why: R3 stays small; the first orderBy field is the only place where a range is one seek and walk of the index.
- Instead of: Adding range({ from, to }) with the named form.
- Topics: rfc-0018, indexes, queries

`docs/rfc/0018-model-api-for-placement.md` states it (NOC-551).
