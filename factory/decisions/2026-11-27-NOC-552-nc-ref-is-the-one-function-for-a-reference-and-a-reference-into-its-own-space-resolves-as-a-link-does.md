# 2026-11-27 · NOC-552 · nc.ref is the one function for a reference, and a reference into its own space resolves as a link does

- Decided by: Carsten, 2026-11-27, on RFC 0018's open questions, through pm (NOC-552)
- Why: The model reads the same whatever the space cut; an in-space reference keeps a link's five statuses and may be followed by computed fields, and nc.ref over a field locked as ncdb/link keeps that type, so no data or lock migration is needed.
- Instead of: Keeping two functions as today, or one function whose in-space references keep a reference's three states.
- Topics: rfc-0018, references, links, public-api, schema

`docs/rfc/0018-model-api-for-placement.md` states it (NOC-551).
