# 2026-11-08 · NOC-27 · One file per decision in factory/decisions/, with one command to read them all

- Decided by: Carsten
- Why: every PR appended to the end of `factory/DECISIONS.md`, so each merge put every other open PR into conflict there, and GitHub runs no CI on a conflicted PR. One file per decision removes the conflict at its source, and `pnpm decisions` makes the log easier to read than one long file; the existing entries moved into the folder too, so there is one place to read.
- Instead of: keeping one file and merging main in after every merge, or writing decisions only in the PR description for pm to copy.
- Topics: factory, decision-log
