---
type: feature
tickets: [NOC-250, NOC-84]
components: [sdk-rust, core]
---
## Problem
The core could fold a plugin field, but the Rust SDK had no way to hand it a plugin, to write such a field or to read it. A field whose plugin was missing stayed parked even after the plugin was added.

## Use
Hand each plugin to `OpenOptions::plugin`, once per plugin, as the plugin's crate exports it from `plugin()` (RFC 0011). A plugin field reads as `Data` and is written with a `Data` intent, which the plugin's `write` turns into operations:

```rust
let db = Db::open(OpenOptions::new(LOCK).path("./data").plugin(acme_ncdb_counter::plugin())).await?;
space.write(|tx| tx.update("tasks", task, [("votes", Value::from(Data::Integer(1)))])).await?;
let votes: Option<Data> = space.get("tasks", task).await?.unwrap().get("votes")?;
```

A write to a plugin field whose type and rule no plugin given implements is refused with `ErrorCode::UnknownType`, which names the type id and the rule; the field reads as unset, and its operations park as `ParkReason::UnknownType`. Opened with the plugin later, the database folds what was parked. An intent the plugin refuses is `InvalidValue`, with the plugin's reason. Two plugins of one namespace, or one of `ncdb`, are refused at open with `InvalidValue`.
