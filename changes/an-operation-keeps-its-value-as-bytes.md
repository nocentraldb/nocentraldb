---
type: breaking
tickets: [NOC-422]
components: [core]
---
## Problem

A decoded operation of a change held its value (key 5) as a tree of `cbor::Value`, which takes about twenty times the bytes of its encoding; a large change took many times its size in memory on every device that wrote or received it.

## Use

`ncdb_core::change::Operation` keeps its value as the canonical bytes the change carries. `Operation::value()` decodes it, and `Operation::has_value()` says whether there is one. `Operation::new` takes the value as before.

## Migration

Read `op.value()` instead of `op.value`, and `op.has_value()` instead of `op.value.is_some()`. The field can no longer be set directly; build an operation with `Operation::new`. Only code that uses `ncdb-core` directly is affected; the SDKs' APIs do not change.
