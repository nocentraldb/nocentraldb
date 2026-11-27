# 2026-11-27 · NOC-552 · An index declares its equality fields with where and its order with orderBy, and today's form stays valid for good

- Decided by: Carsten, 2026-11-27, on RFC 0018's open questions, through pm (NOC-552)
- Why: The named form builds the same lock entry byte for byte, so no lock, model hash or cursor changes, and no existing model has to change.
- Instead of: Warning about the old form in the build, or removing it in a later major release.
- Topics: rfc-0018, indexes, public-api, schema

`docs/rfc/0018-model-api-for-placement.md` states it (NOC-551).
