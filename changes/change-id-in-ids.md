---
type: breaking
tickets: [NOC-248]
components: [core]
---
## Problem

`ncdb-core`'s change id, operation id, member id and clock value move to the new crate `ncdb-plugin`, since a plugin field type sees them (RFC 0011). A Rust type's own functions must live in the crate that defines it, so `ChangeId::compute`, which hashes a change, could not stay on it without putting the hash of a change into the plugin's API.

## Use

`ncdb_core::ids::{ChangeId, MemberId, OperationId}`, `ncdb_core::clock::Hlc`, `ncdb_core::field::Outcome`, `ncdb_core::field_op::Origin` and `ncdb_core::cbor` keep their paths, as re-exports of `ncdb-plugin`'s. A change id is computed with `ncdb_core::ids::change_id`.

```rust
let id = ncdb_core::ids::change_id(frame_version, &header_bytes);
```

## Migration

`ChangeId::compute(version, bytes)` becomes `ids::change_id(version, bytes)`, and `ChangeId::CONTEXT` becomes `ids::CHANGE_ID_CONTEXT`.
