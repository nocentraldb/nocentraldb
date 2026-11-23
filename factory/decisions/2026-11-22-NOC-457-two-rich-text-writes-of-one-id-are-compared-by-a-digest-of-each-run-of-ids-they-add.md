# 2026-11-22 · NOC-457 · Two rich-text writes of one id are compared by a digest of each run of ids they add, the same however an update cuts its items

- Decided by: coding agent (NOC-457)
- Why: RFC 0013's rule for the same id twice needs to tell equal content from different content, and the state keeps no deleted content. Each operation records, per run of ids it adds, a SHA-256 over its content that does not change when an item is cut in two. Two operations conflict when their runs overlap other than as equal runs with equal digests; of the two, the later in stamp order parks as malformed arguments.
- Instead of: comparing the overlapping items themselves, which needs the earlier update's bytes; keeping every update in the state, which makes the state as large as the log; a digest per clock, which grows the state with every character.
- Topics: rich-text, rfc-0013, convergence, yjs

Two writes that carry the same items cut into runs of other lengths conflict; an honest member writes each id once through its lease, so only a restored member or a forged write meets this.
