# 2026-11-15 · NOC-320 · Copies may come from several spaces; a deleted object is not copied

- Decided by: Carsten, 2026-11-15, on NOC-320 (Q7)
- Why: a task can need objects of more than one space, and the mapping keeps each source's space. A deleted object is refused with `InvalidValue`. Every copy gets a fresh object id.
- Instead of: one source space per ad-hoc space, which would split one task over several ad-hoc spaces.
- Topics: sub-agents, ad-hoc-spaces
