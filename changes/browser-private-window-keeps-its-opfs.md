---
type: fix
tickets: [NOC-444]
introduced: cycle
components: [sdk-ts]
---
## Symptom
In a Chromium private window, or in Playwright's `browser.newContext()`, a database that went about ten minutes without a call failed its next write with `storageFailed: the local view failed: SQLite failed: disk I/O error`. Opened again, it was empty. An open that ran longer than ten minutes, as on 0.1.0-beta.1 with about 1,000 persons, failed the same way at its end.

## When
Only in Chromium's off-the-record contexts: a private window, and every `browser.newContext()` in Playwright. Normal windows and Playwright's persistent contexts were not affected. Neither were Firefox and WebKit, which refuse to open a database in a private window.

## Cause
In an off-the-record context, Chromium keeps the origin's OPFS in memory. It drops it after about ten minutes without a file operation, with every access handle and every file in it. The worker held the database's OPFS handles open, but nothing touched them while the app made no call.

## Fix
While a database is open, its worker touches OPFS once a minute, from the start of the open. That keeps the files and handles alive in a private window, and costs one call a minute elsewhere. When the app closes the database, the keep-alive stops. A Chromium private window that then stays without an open database for ten minutes still loses its copy.
