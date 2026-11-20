# 2026-11-08 · NOC-12 · The view relies on the device's disk encryption in v1, and SQLite overwrites deleted content

- Decided by: Carsten (RFC 0006 decisions 6 and 9)
- Why: a key held on the same unlocked device protects little more than the OS's disk encryption, the platform keystore and iOS's "complete until first user authentication"; the SDK documents what that does not cover. Erasure should remove what the database can reach on a device that comes back online, so `secure_delete` is on and the WAL is checkpointed with `TRUNCATE` after a shred or after leaving a space; the limits on flash storage stay, and the SDK says so.
- Instead of: encrypting the database file with SQLCipher or SQLite3 Multiple Ciphers under a key in the platform keystore, or deciding later, after v1 (6); SQLite's defaults, where free pages keep old content until reused (9).
- Topics: rfc-0006, local-view, sqlite, erasure, crypto

`docs/rfc/0006-local-view.md`, "What the view stores in plaintext", states what a device holds and what this does not cover.
