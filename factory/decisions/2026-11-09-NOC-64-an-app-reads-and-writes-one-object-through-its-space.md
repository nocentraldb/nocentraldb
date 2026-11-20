# 2026-11-09 · NOC-64 · An app reads and writes one object through its space handle, and space.write records exactly one change

- Decided by: Carsten, 2026-11-09, in chat with pm, on #78 (RFC 0009 decisions S2 and S3)
- Why: An object id is unique only in its space, every write is one change in one space and every right is per space, so the space is the natural receiver; queries span spaces through `db`, because RFC 0006 chose one database per member for that. `space.write(tx => ...)` is a synchronous recorder whose writes become one change, resolved when folded on the device, so the app sees exactly what is atomic; whether the relay holds the change is asked through `db.sync`.
- Instead of: everything on `db` with an object reference `{ space, id }` (S2 B); an async callback that may read inside the transaction, which suggests an isolation that does not exist (S3 B); only single calls, each one change, so "create a task and link it" could reach another member apart (S3 C); a write that resolves only when the relay accepts it; a compare-and-set write; transactions across spaces.
- Topics: rfc-0009, sdk, api, write, spaces
