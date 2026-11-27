# 2026-11-27 · NOC-572 · The SDK waits 30 s for each call of the app's relay function, then reports `relay_url_failed` and calls it again

- Decided by: Carsten, 2026-11-27, on NOC-572 ("Ich stimme der Empfehlung zu.")
- Why: a token fetch that hangs (a network stall, a lost callback) should not stop syncing in silence; after 30 s per call both SDKs end the attempt as `relay_url_failed`, retry with the usual backoff and call the function again, and a late answer of the old call is ignored. 30 s is long for a token fetch and short enough to notice. Built in NOC-585, natively and on the browser's event loop; the tests shorten the bound only through a test-only hook.
- Instead of: no timeout, with the RFC saying that a function that never settles stops syncing; or a timeout the app sets as an open option, which nobody asked for yet.
- Topics: sdk-api, sdk-rust, sdk-ts, relay, sync
