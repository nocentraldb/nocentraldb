# 2026-11-09 · NOC-34 · db.durability reports what a copy would lose, and the SDK asks for persistent storage on open without waiting

- Decided by: Carsten, 2026-11-09, in chat with pm, on #73 (RFC 0006 decisions 10 and 11, which refine decision 8 of 2026-11-08 on NOC-12)
- Why: eviction loses the member's unconfirmed writes, and the developer must learn that from the SDK, not from a user (decision 8). One namespace, `db.durability`, the same on every target, with `current()`, `subscribe(listener)` and `requestPersistent()`, answers what this user would lose if the browser deleted the copy now; only the core knows which changes the relay has confirmed (10). The SDK asks on open without waiting, and `nc.open({ model, persistentStorage: "on-request" })` lets an app ask after its own onboarding text (11).
- Instead of: leaving the request to the app (8); two places, `db.storage.persistent` and `db.sync.pending`, or a boolean `db.isPersistent` only (10); `nc.open` waiting for the answer, which hangs in Firefox until the user answers the popup, or asking only when the app calls `requestPersistent()` (11).
- Topics: rfc-0006, browser, durability, api

`docs/rfc/0006-local-view.md`, "What the SDK reports", states the fields.
