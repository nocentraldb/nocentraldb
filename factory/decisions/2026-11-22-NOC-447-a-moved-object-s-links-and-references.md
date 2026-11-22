# 2026-11-22 · NOC-447 · A move clears and counts links to objects left behind, links to the moved object read moved, and references follow the trace

- Decided by: Carsten, 2026-11-22, on RFC 0016's questions, through pm (NOC-449)
- Why: a link stays inside its space, as an ad-hoc copy already treats it; the moved status is a function of the old space's copy, so it converges; reference resolution is already local to each member.
- Instead of: refusing a move until every link target moves along; reading a moved target as deleted; references that read missing until rewritten.
- Topics: move, links, references, rfc-0016
