---
type: feature
tickets: [NOC-139]
components: [core, sdk-rust]
---
## Problem
A member added to a space after its key was rotated holds a key envelope for the current generation only. The Rust SDK did not open the older generations' keys, so `spaces().get` answered `NotFound` for that space for good, and the member could not read the changes sealed before it joined.

## Use
Nothing changes in the calls: once the member's copy holds its envelope, `spaces().get` gives the handle and reads see the whole history. The SDK walks the space's key chain back to the first generation.

```rust
let project = db.spaces().get(space_id).await?;
let older = project.get("tasks", task_id).await?; // sealed before the member joined
```
