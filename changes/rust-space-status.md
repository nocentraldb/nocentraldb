---
type: feature
tickets: [NOC-190]
components: [core, sdk-rust]
---
## Problem
A Rust app could not tell what a space holds that a read does not show. It could not see whether the member's role changed, how far the space is synced, which changes are parked or rejected and why, which of its own changes the relay refused, or whether the space waits for a key.

## Use
`space.status().current()` gives a `SpaceStatus` with `role`, `initial_sync_complete`, `behind`, `gaps`, `parked` and `rejected` counted per reason, `refused`, `lost_grants`, `exposed` and `waiting_for_key`. `space.status().subscribe()` is a stream of it, at once and after every change.

```rust
let status = project.status().current();
if !status.parked.is_empty() {
    show("Some data needs a newer version of this app");
}
```

The status does not report the newest model version among parked changes, the "another model" reason or shreds that wait yet; each comes when it is built.
