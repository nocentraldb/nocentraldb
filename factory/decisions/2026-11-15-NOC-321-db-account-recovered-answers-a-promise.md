# 2026-11-15 · NOC-321 · db.account.recovered() answers a promise

- Decided by: Carsten, 2026-11-15, on NOC-321 (A)
- Why: RFC 0009 names `db.account().recovered()` for Rust and leaves its TypeScript form open. `db.account.recovered()` returns `Promise<Notice | null>`: `accountRecovered` on the open that recovered the account, else `null`, as the core answers it. RFC 0009's "Accounts and devices" says so.
- Instead of: a synchronous answer from open's result (option B).
- Topics: sdk-ts, accounts, recovery
