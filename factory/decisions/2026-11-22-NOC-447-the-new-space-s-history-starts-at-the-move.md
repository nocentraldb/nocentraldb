# 2026-11-22 · NOC-447 · A moved object's history in the new space starts at the move; the earlier history stays in the old space

- Decided by: Carsten, 2026-11-22, on RFC 0016's questions, through pm (NOC-449)
- Why: the new space's members read the state, not the drafts, and every earlier change stays verifiable in the space it was signed in.
- Instead of: the mover restating the history into the new space, unverifiable there; or giving the old space's keys to the new space's members.
- Topics: move, history, rfc-0016
