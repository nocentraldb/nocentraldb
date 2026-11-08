# 2026-11-08 · NOC-27 · pnpm decisions refuses to print a folder the gate would reject

- Decided by: coding agent (NOC-27)
- Why: the reading command and the gate share one parser; when a file is malformed, `pnpm decisions` prints the same problems as the gate and exits 1, so a reader never sees a log that silently drops or misreads an entry.
- Instead of: printing what it can parse and skipping the rest, which could hide a decision from an agent that reads the log before it starts.
- Topics: factory, decision-log
