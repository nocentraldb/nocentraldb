# 2026-11-27 · NOC-552 · A move writes references among the moved objects with the new space, and keeps references to objects left behind

- Decided by: Carsten, 2026-11-27, on RFC 0018's open questions, through pm (NOC-552)
- Why: A reference between two objects moved together then reads ok for a member of the new space only, as a link does today, and nothing is cleared.
- Instead of: Keeping every reference as written, as RFC 0016 decided before.
- Topics: rfc-0018, rfc-0016, references, move

`docs/rfc/0018-model-api-for-placement.md` states it (NOC-551).
