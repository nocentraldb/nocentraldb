# 2026-11-22 · NOC-447 · A moved object's trace shows no fields in the old space's view, while its members keep the log, and the move returns the notice objectMoved

- Decided by: Carsten, 2026-11-22, on RFC 0016's questions, through pm (NOC-449)
- Why: the object leaves the old space's queries and indexes as the app means it to, and the notice says plainly that the old members keep the history and the state at the move.
- Instead of: a trace that keeps its last state readable in the view.
- Topics: move, local-view, notices, rfc-0016
