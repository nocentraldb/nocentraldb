---
type: breaking
tickets: [NOC-518]
components: [core]
---
## Problem

`ncdb-core`'s `Log` kept a whole header for every change it holds, about 140 bytes each, only so that `Log::header` could lend it out. With one record to a change, the log's bookkeeping was most of what an open space held in memory.

## Use

`Log::fields(id)` returns a `change::HeaderFields`: the space, the author, the sequence, the key generation, the kind, the format and the suite of the change's header. It has no payload and no unknown keys, so it cannot be mistaken for the header. `Header::fields()` gives the same of a whole header. The membership verdict reads either.

```rust
let fields = log.fields(id).expect("the log holds the change");
let admitted = checks.resolved().admits(&fields);
```

## Migration

`Log::header` is removed: call `Log::fields(id)` for the header's fields, or `Log::change(id, frames)` for the whole change and its header. `Checks::may_unfold` takes `&HeaderFields` instead of `&Header`. `Resolved::admits` and `MemberState::admits` take `impl Into<HeaderFields>`, so a `&Header` still works.
