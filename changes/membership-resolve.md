---
type: feature
tickets: [NOC-67]
components: [core]
---
## Problem

`ncdb-core` had no way to start a space or to tell who belongs to it: RFC 0008's genesis, membership entries and its membership function `resolve` were not built.

## Use

`membership` encodes and decodes the genesis and the membership entries (add, set role, remove, with cutoffs) as control bodies, with RFC 0003's member certificate, the derived space id and generation ids, and the sealed model id. `resolve` folds the control changes of a space into its membership, the same in any order of arrival:

```rust
let state = membership::resolve(space, log_changes);
state.role(member);          // Some(Role::Writer), or None
state.verdict(change.id());  // Accepted, Lost, Rejected(reason) or Parked
```

Re-adding a removed member, or giving the right to write back, is rejected until key chain entries are folded.
