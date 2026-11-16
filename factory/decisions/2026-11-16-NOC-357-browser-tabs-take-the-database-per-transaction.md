# 2026-11-16 · NOC-357 · Browser tabs take the database per transaction, and no tab owns it (option B of RFC 0012)

- Decided by: Carsten, 2026-11-16, on RFC 0012's first question, through pm
- Why: a user does not know which tab is "the right one", expects every tab to sync while it is online, and "another tab is asleep" would make every app look broken. With a lock per transaction on `opfs-sahpool` and one syncing tab whose lock the visible tab may steal, a frozen tab blocks the others only if it froze inside a transaction of a few milliseconds.
- Instead of: keeping one owning tab with NOC-333, NOC-352, NOC-355 and NOC-356 (A, not taken); adding an outbox in IndexedDB now (C, deferred until telemetry gathered with the app's consent shows whether it is needed, NOC-364); the note's VFS `opfs-wl` or `OPFSCoopSyncVFS` (D, not taken). What the user sees while a tab waits is the optimistic UI layer's, designed in its own RFC (NOC-363). Questions 2 to 5 of RFC 0012 stay open.
- Topics: browser, tabs, rfc-0012, sdk-ts
