# 2026-11-15 · NOC-326 · The second account's sign-off travels as text the app carries

- Decided by: Carsten, 2026-11-15, on NOC-326 (Q3 a)
- Why: a sign-off binds the space, the person-key id and the performer (RFC 0003, "Account sign-off of a shred"), so each space needs its own, usually from a device of another person. Three calls, no wire change: `db.erasure_request(&object)` returns an `ErasureRequest`, with the text form `ncdb-erase-1:…`, listing per space the space, the person-key id and the performer, and nothing personal; a device of another account calls `db.account().sign_off(&request)`, which signs each space where its account may sign off by RFC 0008 and reports the others; `db.erase(&request, &sign_offs)` writes one shred per signed space and refuses a sign-off that would not hold before it writes anything. The app carries both texts, as it carries invitations and pairing answers.
- Instead of: a request entry and a sign-off entry in the log, a new entry type and an RFC 0008 change that belong to the work-order RFC; and only self-signed, provisional erasure in v1, which an agent cannot perform, since it holds no account key.
- Topics: sdk-rust, erasure, accounts
