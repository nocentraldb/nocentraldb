# 2026-11-23 · NOC-475 · Later edits are computed from the old space's log on demand, not kept in the view, and come in clock order

- Decided by: coding agent, NOC-475
- Why: the trace keeps no field state in the view; later edits are a function of the folded changes and their dependencies, so `moves::later_edits` opens the log's changes when the app asks (`space.moves.laterEdits`, `space.inspect`) and every member gets the same list, ordered by stamp, author and id. Edits in the winning move's own change belong to its past; a losing move is a conflict of the move, not a later edit.
- Instead of: a table of later edits the fold maintains, which would hold the trace's plaintext and need its own un-folding.
- Topics: rfc-0016, move, later-edits, view, convergence
