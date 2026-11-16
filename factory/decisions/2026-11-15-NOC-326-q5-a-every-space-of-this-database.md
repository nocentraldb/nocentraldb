# 2026-11-15 · NOC-326 · "Every space the account reaches" is every space of this database where it may write

- Decided by: Carsten, 2026-11-15, on NOC-326 (Q5 a)
- Why: one database is one member, and a member writes only in its own spaces. `Db::erase` erases in every space of the database where the member is a writer and the person has a key, and its notice says that other devices and agents of the account erase only when they run the erasure too. An ad-hoc copy is reached, because the parent agent is a member of the source space and of the copy.
- Instead of: fanning the erasure out through the account space to every device of the account, which is a work order and waits for the work-order RFC.
- Topics: sdk-rust, erasure, accounts
