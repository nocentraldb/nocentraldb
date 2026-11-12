# 2026-11-12 · NOC-244 · A connected session fetches a missing person key again

- Decided by: coding agent (NOC-244), as pm asked
- Why: RFC 0009 says a personal value whose key this device has not fetched yet reads as unset and "reads again once the key arrives", but the session fetched person keys only when it connected, so a person drawn by another device during the connection never reached this one. While connected, the session now fetches a space's person-key records again whenever the log names a person key the member does not hold and that is not destroyed: at once, then after 250 ms, with the wait doubling up to the session's idle interval while the key stays missing. The writer puts the record just after it pushed the entry, so the record usually lands within the first retries.
- Instead of: fetching only on connect, which leaves the value unreadable until the next reconnect; and a fetch on every incoming entry without a wait, which repeats the request at once while the record is not stored yet.
- Topics: personal-fields, sync, person-keys
