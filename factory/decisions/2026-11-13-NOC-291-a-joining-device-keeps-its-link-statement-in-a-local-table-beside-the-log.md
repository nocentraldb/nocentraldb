# 2026-11-13 · NOC-291 · A joining device keeps its link statement in a local SQLite table beside the log until the relay confirms it in the account space

- Decided by: Carsten (2026-11-13, on NOC-291, point 1, option A)
- Why: at `complete_pairing` the new device does not hold the account space yet, and once the old account key is deleted nobody can sign the statement again, so a lost waiting copy loses the link for good. The table sits in the database's SQLite file beside the log, not in the view, so rebuilding the view never drops it. The row is written before the key record that drops the old key is saved, and removed if that save fails. The SDK writes the statement into the account space's `links` once it holds the account space, and deletes the row only after the relay confirms that change. It is local only: no wire format, and the key record is unchanged.
- Instead of: keeping the old account key until the statement is written and signing it then (B), which needs a field in the key record (format 3 or a change to format 2) and keeps an old key on disk longer.
- Topics: accounts, linking, pairing, sqlite, on-disk-format, rfc-0010
