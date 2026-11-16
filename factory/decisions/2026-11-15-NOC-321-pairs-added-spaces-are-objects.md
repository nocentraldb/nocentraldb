# 2026-11-15 · NOC-321 · pair's added spaces are objects

- Decided by: Carsten, 2026-11-15, on NOC-321 (A)
- Why: RFC 0009 says `db.account.pair` returns `{ answer, added, askAdmins }` in TypeScript, Rust's `Paired`, whose `added` is `Vec<(SpaceId, Notice)>`. In TypeScript `added` is `[{ space, notice }]`, objects as the rest of the TypeScript API returns them. RFC 0009's "Accounts and devices" says so.
- Instead of: `[space, notice]` tuples, as in Rust (option B).
- Topics: sdk-ts, accounts, pairing
