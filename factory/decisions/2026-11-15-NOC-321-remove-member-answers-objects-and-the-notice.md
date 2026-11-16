# 2026-11-15 · NOC-321 · removeMember answers objects and the notice

- Decided by: Carsten, 2026-11-15, on NOC-321 (A)
- Why: RFC 0009 says `db.account.removeMember(memberId, { recover: words })` in TypeScript, and Rust's `Account::remove_member` returns `Removed { removed: Vec<(SpaceId, Notice)>, ask_admins: Vec<SpaceId>, notice: Notice }`. In TypeScript it returns `{ removed: [{ space, notice }], askAdmins, notice }`: Rust's `Removed`, with its spaces as objects as `pair`'s `added` are (NOC-321 point 4). RFC 0009's "Accounts and devices" says so.
- Instead of: no other option was put to the owner for this point; `[space, notice]` tuples, as in Rust, were not taken for `pair` (point 4, option B), and so not here.
- Topics: sdk-ts, accounts
