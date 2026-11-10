---
type: breaking
tickets: [NOC-78]
components: [core]
---
## Problem

`ncdb-core`'s `SealChecks` learned member keys and space keys only when the caller handed them over, and its membership came from a trait the caller implemented. A change that arrived before the membership entry or the key envelope it needed was not checked again when they arrived.

## Use

`SealChecks::new(space, member_key)` takes the space and the member's own key, and learns everything else from the space's control changes as the log receives them: the authors' public keys from the genesis and the add entries, the membership from `membership::resolve`, and the space keys from the key envelopes to the member, which it opens. Step 4 uses RFC 0008's membership verdict for a sealed change, `Resolved::admits`. A change rejected for an unknown author, a missing membership or a missing key is folded once the control change it lacked arrives. `SealChecks::resolved` gives the control state. A key envelope reads as `membership::Entry::KeyEnvelope`.

```rust
let mut checks = SealChecks::new(space, member_key);
log.receive(&frame, &mut checks, &mut fold)?;
```

## Migration

`SealChecks` has no type parameter, and `SealChecks::add_member`, `SealChecks::add_space_key`, `SealChecks::membership` and the trait `checks::Membership` are removed: deliver the genesis, the membership entries and the key envelopes to the log instead. A `match` on `membership::Entry` needs an arm for `Entry::KeyEnvelope`; entry type 2 no longer reads as `Entry::Other`.
