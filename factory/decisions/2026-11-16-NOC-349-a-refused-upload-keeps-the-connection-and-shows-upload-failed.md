# 2026-11-16 · NOC-349 · An upload the URL refuses keeps the connection and shows `upload_failed`

- Decided by: Carsten, 2026-11-16, NOC-349 (option A)
- Why: a refused upload used to end the connection, and the member reconnected and asked for a new upload URL without end, Offline, with nothing for the app to show. Now the connection stays up, the change stays unconfirmed and goes up again after a backoff, and `SyncStatus.error` shows the new code `upload_failed`, which RFC 0009 lists.
- Instead of: the same behaviour with no code, which leaves the failure silent; or reusing an existing code, none of which fits.
- Topics: sync, sdk, large-changes, status
