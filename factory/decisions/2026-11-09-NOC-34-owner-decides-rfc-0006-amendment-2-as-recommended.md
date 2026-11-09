# 2026-11-09 · NOC-34 · The owner decides RFC 0006 Amendment 2, decisions 10 to 15, as recommended

- Decided by: Carsten, 2026-11-09, in chat with pm, on #73
- Why: the SDK reports a copy's durability through `db.durability` (`storage`, `unconfirmed`, `oldestUnconfirmedAt`, `atRisk`, `subscribe`, `requestPersistent`); it asks for persistent storage on open without waiting, or later with `persistentStorage: "on-request"`; a private window without OPFS keeps the in-memory VFS; one page, "Browser apps and durability", explains it; the page carries the English text for end users and the SDK ships none; the README and RFC 0001 link the page, and the SDK warns once per page load in development. The device checks of 2026-11-09 (#74) changed no recommendation.
- Instead of: two separate status APIs or a boolean only (10 B, C); waiting for `persist()` in `nc.open` or asking only on request (11 B, C); a VFS on IndexedDB in a private window (12 B); the facts only in the API reference (13 B); texts exported by the SDK (14 B); a warning in every build, or none (15 B, C).
- Topics: browser, durability, rfc-0006, api, docs, user-text
