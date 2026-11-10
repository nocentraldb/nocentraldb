---
type: breaking
tickets: [NOC-71]
components: [core]
---
## Problem

`ncdb-core` rejected every re-grant: a removed member could not be added again, and a reader could not be raised to writer. Its membership verdict for sealed changes counted a change only under its author's own from generation, so a writer's changes after a key rotation were rejected as "not a member".

## Use

Key chain entries (RFC 0008, entry type 3) read as `membership::Entry::KeyChain`, and `membership::resolve` builds the generation graph from them: `Resolved::generation`, `Resolved::generations` and `Resolved::descends`. An admin re-grants by rotating first and naming the new generation as the from generation of the add or set role; `resolve` accepts it when no key envelope to the member for that generation came before. `Resolved::admits`, and so `SealChecks`, counts a sealed change under any generation that descends from its author's grant. Step 1 now also checks a key chain entry's id and previous generations, and a key envelope's recipient and generation.

```rust
let state = membership::resolve(space, &control_changes);
assert!(state.descends(new_generation, first_generation));
```

## Migration

A `match` on `membership::Entry` needs an arm for `Entry::KeyChain`; entry type 3 no longer reads as `Entry::Other`. A `match` on `membership::Rejection` needs an arm for `Rejection::DuplicateGeneration`, given to two accepted key chain entries that name the same generation id.
