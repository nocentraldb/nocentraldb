# 2026-11-23 · NOC-475 · Later edits are computed from the old space's log on demand, not kept in the view; what the app sees is unchanged

- Decided by: pm, 2026-11-23, on #462
- Why: the trace keeps no field state in the view, and storage is internal: `space.inspect` on the trace and `space.moves.laterEdits` must return later edits exactly as RFC 0016 describes, after a reopen as well. `moves::later_edits` computes them from the folded changes and their dependencies, so every member gets the same list, ordered by stamp, author and id. Edits in the winning move's own change belong to its past; a losing move is a conflict of the move, not a later edit. NOC-474 words RFC 0016 accordingly ("shown by `space.inspect`", not "folded onto the trace"). Should `inspect` ever show less than the RFC describes, that is the owner's call.
- Instead of: a table of later edits the fold maintains, which would hold the trace's plaintext and need its own un-folding.
- Topics: rfc-0016, move, later-edits, view, convergence
