# 2026-11-16 · NOC-355 · A tab that waits for another tab's storage reports `waitingForTab` on `db.durability`, after 1 s

- Decided by: Carsten (chat with pm, 2026-11-16)
- Why: in Safari a write waits while a frozen tab still holds the storage; the app must learn it to tell the user. The worker knows exactly when it waits for the storage lock, and a 1 s grace hides the few milliseconds a normal handover takes (measured in Firefox), while the worker's timer still fires behind a frozen owner in WebKit (measured).
- Instead of: `waitingFor: "anotherTab" | null`; a report at once with a delay left to each app; a field on `db.sync`; a report of its own (`db.tabs`); a flag tied to a pending write rather than to the wait itself
- Topics: sdk-ts, browser, durability, public-api
