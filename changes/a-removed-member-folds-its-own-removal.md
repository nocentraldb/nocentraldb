---
type: feature
tickets: [NOC-383]
components: [relay, sdk-rust]
---
## Problem
A member removed while it was offline, and kept writing, never received its own removal: the relay served it nothing of the space. Its own copy kept its late writes as if they counted, while every other member rejected them, and the app could only say that the member was removed, not which of its writes were lost.

## Use
Nothing to call. When the relay refuses a removed member's pull or subscription, its error carries the control change that removed the member, and nothing else of the space. The SDK folds it before it stops syncing the space, so the member's writes after the cutoff become rejected on its own copy too, and `space.status()` counts them under `rejected` with `NotMember`:

```rust
let status = space.status().current();
let lost = status.rejected.get(&RejectReason::NotMember).copied().unwrap_or(0);
```
