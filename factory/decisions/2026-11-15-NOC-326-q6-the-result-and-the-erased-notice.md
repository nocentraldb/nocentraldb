# 2026-11-15 · NOC-326 · What `Db::erase` returns, and the notice `Erased`

- Decided by: Carsten, 2026-11-15, on NOC-326 (Q6)
- Why: the app needs each space's shred, to match it in `space.erasures().list()`, whether it is provisional, and the spaces it could not erase in. `Erased { shreds, skipped, notice }`: each shred with its space, its change id and whether it is provisional; each skipped space with why, no sign-off, not a writer there, or no key of the person there. The notice is `NoticeCode::Erased` with the text "The person's personal fields are erased in these spaces. Devices that never come back online, copies made outside the database and unmarked free text keep them; other devices and agents of this account erase only when they run the erasure too." A notice's text is for the developer, saying what the call does not do; apps match on the code (RFC 0009, "Invitations and members").
- Instead of: one result per space without the skipped ones, which would let an app miss a space it did not erase in.
- Topics: sdk-rust, erasure, notices
