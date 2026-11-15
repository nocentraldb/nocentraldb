# 2026-11-15 · NOC-321 · The TypeScript recovery code is an exported type

- Decided by: Carsten, 2026-11-15, on NOC-321 (A)
- Why: the TypeScript recovery code is an exported type, `RecoveryCode { words(): readonly string[]; confirm(): Promise<void> }`, with the twelve words. Its `toString()`, `toJSON()` and Node's inspect give `"RecoveryCode(redacted)"`, as RFC 0009 says. RFC 0009's "Accounts and devices" names the type.
- Instead of: an interface that is not exported (option B).
- Topics: sdk-ts, accounts, recovery
