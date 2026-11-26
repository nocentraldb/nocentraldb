---
type: breaking
tickets: [NOC-518]
components: [core]
---
## Problem

`ncdb-core`'s `Log` kept a whole header for every change it holds, about 140 bytes each, only so that `Log::header` could lend it out. With one record to a change, the log's bookkeeping was most of what an open space held in memory.

## Use

`Log::header(id)` returns the header as a value, built from the fields the log keeps: the space, the author, the sequence, the key generation, the kind, the format and the suite. Its payload is empty and it holds none of the header's unknown keys, whether the log still holds the change or released it. `Log::change` and `Log::change_at` still give the whole change.

```rust
let header = log.header(id).expect("the log holds the change");
let admitted = checks.resolved().admits(&header);
```

## Migration

`Log::header` returns `Option<Header>` instead of `Option<&Header>`: borrow the value where a reference is needed (`&header`). Read a change's payload or its unknown header keys from `Log::change(id, frames)` instead.
