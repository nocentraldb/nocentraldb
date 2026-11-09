# 2026-11-09 · NOC-69 · The view keeps each space's log in memory beside the log table

- Decided by: coding agent (NOC-69)
- Why: the steps of RFC 0002 Amendment 1, D11's re-check and the states live in `Log::receive` on the in-memory `Log`. `View::receive` runs them unchanged, inside the SQLite transaction that writes the frame, the states the call changed and the fold; opening the view reads every space's log back from the `log` table, and a failed receive reloads it, so the table stays the source of truth. The cost is a second copy of the frames in memory.
- Instead of: rewriting the steps against SQLite now, which would duplicate `receive` or move it behind a storage trait in a ticket about the view; a later ticket can do that when a log outgrows memory.
- Topics: rfc-0002, rfc-0006, view, log
