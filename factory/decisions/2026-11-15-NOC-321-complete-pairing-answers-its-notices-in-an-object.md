# 2026-11-15 · NOC-321 · completePairing answers its notices in an object

- Decided by: Carsten, 2026-11-15, on NOC-321 (A)
- Why: Rust's `complete_pairing` returns `Vec<Notice>`. In TypeScript `db.account.completePairing(answer)` returns `Promise<{ notices: Notice[] }>`, as the other calls return `{ notice }`. RFC 0009's "Accounts and devices" says so.
- Instead of: `Promise<Notice[]>` (option B).
- Topics: sdk-ts, accounts, pairing
