# 2026-11-15 · NOC-326 · The erase call is `Db::erase`, across the database's spaces

- Decided by: Carsten, 2026-11-15, on NOC-326 (Q1 a)
- Why: an erasure shreds a person's keys in every space the account reaches (RFC 0003, "The person key"), so the call lives on the database, not on one space. RFC 0009 leaves the erase call to the work-order RFC; until that RFC, `Db::erase` is the Rust SDK's call, and the work-order RFC may wrap it.
- Instead of: `db.work_orders().erase`, RFC 0003's sketch, which presumes a work-order design that does not exist yet; and `space.erase`, which leaves the loop over spaces to every app.
- Topics: sdk-rust, erasure
