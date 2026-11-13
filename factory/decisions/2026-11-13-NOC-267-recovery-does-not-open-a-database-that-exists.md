# 2026-11-13 · NOC-267 · Recovery does not open a database that exists

- Decided by: pm, NOC-267, on the coding agent's proposal
- Why: RFC 0010 recovers an account "on a new device"; `OpenOptions::recover(words)` on a database that already exists fails with `invalidValue`, drops the keys it drew, and leaves the database untouched, so a recovery code can never silently replace an account's keys.
- Instead of: recovering over the existing database's keys, which would replace its account without a word.
- Topics: sdk-rust, accounts, recovery
