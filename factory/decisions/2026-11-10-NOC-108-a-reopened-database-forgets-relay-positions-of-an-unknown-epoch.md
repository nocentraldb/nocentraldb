# 2026-11-10 · NOC-108 · A reopened database forgets the relay positions of an epoch it no longer knows

- Decided by: coding agent (NOC-108)
- Why: the relay positions now persist in the log, while the relay epoch and the cursors stay in memory as NOC-90 left them. A database closed while its relay was restored would keep a change the restore lost marked as confirmed and never push it again, losing it from the relay silently (GENESIS.md, principle 3). So the first `welcome` after each open forgets every relay position, without reporting `epoch_changed`; the cursors start at 0, pulling confirms again every change the relay still holds, and the rest is pushed again.
- Instead of: keeping the epoch and the cursors in the database, which adds to the view's on-disk format, the owner's to decide; or trusting the positions, which loses changes after a restore.
- What this does not do: every open pulls each space from 0, so opening costs time in proportion to the logs until the epoch and the cursors persist.
- Topics: sdk-rust, relay, rfc-0007, local-view
