# 2026-11-27 · NOC-552 · An edge collection is nc.edge(ends, fields, options), as nc.collection takes fields and options

- Decided by: Carsten, 2026-11-27, on RFC 0018's open questions, through pm (NOC-552)
- Why: It mirrors nc.collection, so fields and options never share one object or one set of names.
- Instead of: Nc.edge(ends, { fields, indexes }), or a second argument that mixes fields and options and reserves their names in edges.
- Topics: rfc-0018, edges, public-api, schema

`docs/rfc/0018-model-api-for-placement.md` states it (NOC-551).
