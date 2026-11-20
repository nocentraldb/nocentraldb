# 2026-11-13 · NOC-289 · `db.account().recovered()` gives the AccountRecovered notice on the Db the recovering open returned

- Decided by: Carsten (2026-11-13, on NOC-289, option A)
- Why: NOC-273 decided the notice `AccountRecovered` is returned by the open that recovers an account, but `Db::open` returns a `Db` and has no place for a notice. `db.account().recovered()` returns `Option<Notice>`: `Some(AccountRecovered)` on the `Db` the recovering open returned, `None` on every other open, mirroring `recovery_code()`, which the app already reads after opening. TypeScript gets `db.account.recovered()`.
- Instead of: `Db::open` returning `(Db, Option<Notice>)` when `recover` is set (B), which changes the return type of every open for one case; or the SDK storing the notice until the app reads it once (C), extra storage for a notice about something the person just did themselves.
- Topics: accounts, recovery, notices, public-names, sdk-rust, sdk-ts
