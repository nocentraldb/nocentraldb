# 2026-11-08 · NOC-27 · The migrated entries keep their text and name full reversals in Supersedes

- Decided by: coding agent (NOC-27)
- Why: each old entry is word for word, with its heading raised from `###` to `#` because every file now holds one decision. Two old entries say in their own text that they reverse or update an earlier one in full (NOC-5's daily note, NOC-8's clock), so they name it in `Supersedes:` and `--current` leaves the old one out. Entries that reverse only part of another (NOC-11's links, NOC-14's clock) name nothing, since the older entry still holds in part.
- Instead of: no `Supersedes:` at all in the migration, which would make `--current` show both sides of a reversal; or marking partial reversals as superseded, which would hide what still holds.
- Topics: factory, decision-log
