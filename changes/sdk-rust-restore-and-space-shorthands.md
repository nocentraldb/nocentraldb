---
type: feature
tickets: [NOC-277]
components: [sdk-rust]
---
## Problem

The Rust SDK could delete an object but not restore it, though RFC 0009 lets a deletion be undone. It also had no one-call shorthands: an app wrote even a single create, update or delete through `space.write` and a closure.

## Use

Restore a deleted object in a write, or with the shorthand; each shorthand is one change:

```rust
project.write(|tx| tx.restore("tasks", id)).await?;

let written = project.create("tasks", [("title", Value::from("Ship it"))]).await?;
let task = written.ids[0];
project.update("tasks", task, [("priority", Value::from(2))]).await?;
project.delete("tasks", task).await?;
project.restore("tasks", task).await?;
```

A restore of an object the copy does not hold fails with `NotFound`, as an update or a delete does.
