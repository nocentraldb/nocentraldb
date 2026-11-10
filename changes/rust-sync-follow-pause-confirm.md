---
type: feature
tickets: [NOC-120]
components: [sdk-rust]
---
## Problem
A Rust app could read the sync status only by asking for it again and again, could not keep a database offline on purpose, and could not tell when the relay held one of its writes.

## Use
`db.sync().subscribe()` is a stream of the `SyncStatus`, at once and after every change. `db.sync().pause().await` closes the relay connection until `db.sync().resume().await`. `db.sync().is_confirmed(written.change)` says whether the relay holds a write, and `db.sync().confirmed(written.change).await` waits until it does; it may never resolve, since the relay can refuse the change or never be reached.

```rust
let mut statuses = db.sync().subscribe();
while let Some(status) = statuses.next().await { show(status.connection); }
```
