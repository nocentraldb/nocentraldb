# 2026-11-23 · NOC-496 · The relay option also takes a function that returns a fresh URL

- Decided by: Carsten, 2026-11-23, on NOC-496 ("Ich stimme den Empfehlungen zu.")
- Why: an app that protects its relay endpoint with a short-lived token in the URL (NOC-417) loses sync when the token expires, because the SDK reconnects with the URL it was opened with. The `relay` open option therefore also accepts a function that returns the URL, which the SDK calls before each connect. One option covers a token in the query string or the path and a relay per environment, and it mirrors the `admission` function decided on NOC-493. TypeScript: `relay: string | (() => Promise<string>)`. Rust: `OpenOptions::relay_with(f)` beside `relay(url)`. If the function fails, the sync status reports that reason and the SDK retries with backoff. A refused `$connect` is reported in the sync status as a distinct, plain reason, not as being offline.
- Instead of: a separate `relayToken` option that the SDK appends to the URL; keeping the URL fixed and documenting that the app reopens the database.
- Topics: sdk-api, relay, sync
