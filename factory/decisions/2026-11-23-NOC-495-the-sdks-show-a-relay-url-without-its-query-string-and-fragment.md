# 2026-11-23 · NOC-495 · The SDKs show a relay URL without its query string and fragment, and connect with the whole URL

- Decided by: pm (NOC-495, its acceptance criteria); its form: coding agent (NOC-495)
- Why: a relay protected as NOC-417 documents takes a token in the URL's query string, and apps log their sync status (GENESIS, "Never write content, keys or personal fields into logs"). The TypeScript `SyncStatus.relay` cuts the URL at the first `?` or `#`, as text, without parsing it, so `ws://127.0.0.1:7700` stays exactly as given. The Rust SDK keeps the URL out of `SyncStatus`, its errors and `Debug` altogether. Any new place that reports or prints the relay URL shows it the same way; the connection alone uses the whole URL.
- Instead of: parsing with `URL`, which would change URLs without a path (a trailing `/`), or masking only a parameter named `token`, which misses any other name an app's authorizer reads.
- Topics: sdk-ts, sdk-rust, sync, relay, security
