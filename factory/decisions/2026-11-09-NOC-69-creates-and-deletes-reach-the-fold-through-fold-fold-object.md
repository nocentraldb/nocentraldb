# 2026-11-09 · NOC-69 · Creates and deletes reach the fold through Fold::fold_object

- Decided by: coding agent (NOC-69)
- Why: `Fold::objects` handed out an in-memory map, so a fold that keeps its objects in SQLite could not learn which object a create or delete changed. `Fold::fold_object(object, operation)` replaces it; the in-memory folds apply the operation to their map as before.
- Instead of: keeping `objects()` beside a new method with a default, which every SQLite fold would have to implement with a map it does not have.
- Topics: rfc-0004, rfc-0006, view, fold, api
