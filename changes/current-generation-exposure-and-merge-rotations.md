---
type: breaking
tickets: [NOC-86]
components: [core]
---
## Problem

`ncdb-core` built the generation graph, but did not say which generation a writer seals under, whether a removed member may still hold that generation's key, or how two admins' concurrent rotations come together again.

## Use

`membership::resolve` now chooses the current generation as RFC 0008 defines it: `Resolved::current_generation` is a head of the generation graph (`Resolved::heads`), the one with the highest ordinal, then the one whose key chain entry comes first in the order. A writer seals under it.

Each `Generation` records in `holders` who may hold its key: the author of its key chain entry (the creator for the first generation), the recipient of every key envelope for it, accepted or rejected, and whoever may hold a generation that descends from it. `Resolved::exposed` says whether one of them is no longer a member. When the current generation is exposed, `Resolved::merge_rotation` gives an admin the merge rotation to write: every head as previous, and one envelope for each current member.

```rust
let state = membership::resolve(space, &control_changes);
let generation = state.current_generation().expect("a genesis");
if let Some(rotation) = state.merge_rotation(admin) {
    let entry = rotation.entry(|previous| encrypt_previous_key(previous));
    // write `entry`, then one envelope to each of `rotation.recipients`
}
```

## Migration

`membership::Generation` has a new public field, `holders`. Code that builds a `Generation` with a struct literal adds it, or starts from `Generation::default()`.
