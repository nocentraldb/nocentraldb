# 2026-11-08 · NOC-12 · Indexes sort text by its UTF-8 bytes, and the database commits with WAL and synchronous = FULL

- Decided by: Carsten (RFC 0006 decisions 5 and 7)
- Why: locale-aware collation depends on the platform's ICU version, so two members could order the same page differently; locale-aware order comes only through a computed sort key. The log holds the member's writes the relay has not confirmed, which exist nowhere else, so a committed change must survive a power loss (principle 3).
- Instead of: locale-aware collation in the index; WAL with `synchronous = NORMAL`, faster but losing the last commits before a power loss.
- Topics: rfc-0006, local-view, sqlite

`docs/rfc/0006-local-view.md`, "The order of values" and "SQLite on each target", state the rules.
