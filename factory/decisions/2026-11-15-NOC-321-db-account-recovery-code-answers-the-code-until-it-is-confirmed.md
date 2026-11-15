# 2026-11-15 · NOC-321 · db.account.recoveryCode() answers the code until it is confirmed

- Decided by: Carsten, 2026-11-15, on NOC-321 (A)
- Why: RFC 0009 says the recovery code is handed to the app once, as an object whose string, JSON and debug forms are redacted, but not how a TypeScript app gets it. `db.account.recoveryCode()` returns `Promise<RecoveryCode | null>`, as Rust's `recovery_code()` does: the code on every open until the person confirmed it, then `null`, so an app that crashed before it showed the code shows it on the next open. RFC 0009's "Accounts and devices" says so.
- Instead of: a property set once at open (option B).
- Topics: sdk-ts, accounts, recovery
