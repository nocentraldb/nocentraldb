---
type: feature
tickets: [NOC-119]
components: [sdk-rust, core]
---
## Problem

A member added to a space received and folded its changes through sync, but the Rust SDK had no handle to write into that space, and no way to get the handle of a space the member had created by its id.

## Use

`db.spaces().get(id)` returns the handle of a space the member created or was added to. Its writes depend on every change the member's copy holds, those pulled from others included. A space whose genesis names another model is refused with `ErrorCode::SpaceOfAnotherModel`, which names both model ids; a space the copy does not hold, or whose genesis or keys have not arrived yet, with `ErrorCode::NotFound`.

```rust
let project = db.spaces().get(space_id).await?;
project.write(|tx| {
    tx.create("tasks", [("title", Value::from("Mine"))])?;
    Ok(())
}).await?;
```

In `ncdb-core`, `SealChecks::space_key(generation)` and `SealChecks::model_id()` give the space key of a generation and the genesis's model id, and `View::log(space)` gives the log of a space.
