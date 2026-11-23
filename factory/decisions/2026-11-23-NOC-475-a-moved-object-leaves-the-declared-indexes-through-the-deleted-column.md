# 2026-11-23 · NOC-475 · A moved object leaves the declared indexes through the `deleted` column, which keeps its name

- Decided by: coding agent, NOC-475
- Why: the partial indexes already leave out rows with `deleted = 1`; setting it for a deleted or a moved object takes a trace out of every query with no new column, so no existing view's tables change. The object's state, not the column, says whether it is deleted or moved.
- Instead of: a column `moved` beside `deleted`, which every existing view would have to add and every partial index to name.
- Topics: rfc-0016, move, view, indexes
