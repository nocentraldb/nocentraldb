# 2026-11-27 · NOC-552 · The lock writes a through field as its path, such as project.state, in an index's field

- Decided by: Carsten, 2026-11-27, on RFC 0018's open questions, through pm (NOC-552)
- Why: A field name never holds a dot, so a core that does not know through indexes refuses the lock instead of misreading it.
- Instead of: A separate through key, which an old core would ignore and might index a field of the same name.
- Topics: rfc-0018, model-lock, indexes, format

`docs/rfc/0018-model-api-for-placement.md` states it (NOC-551).
