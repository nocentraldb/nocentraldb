# 2026-11-13 · NOC-267 · A link statement is kept before the key record is saved, and forgotten if the save fails

- Decided by: coding agent (NOC-267), told to pm
- Why: RFC 0010 keeps the joining device's link statement in a store beside its log, written in the same step that completes the pairing (NOC-291 A). The key record lives in a file, a keystore or IndexedDB, not in SQLite, so the two cannot share a transaction. `complete_pairing` writes the pending link row first and deletes it if saving the key record fails, so a completed pairing never lacks its statement and a failed one leaves none behind. Once the database holds the account space, `account::reconcile` writes each kept statement into `links` through the account space's `write()` and forgets it once the relay holds that change, or once `links` already holds it.
- Instead of: saving the key record first, where a crash between the two steps would delete the old account key with no statement to show its memberships as linked.
- Topics: sdk-rust, accounts, pairing, linking
