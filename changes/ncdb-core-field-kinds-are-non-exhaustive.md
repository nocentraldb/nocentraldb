---
type: breaking
tickets: [NOC-441]
components: [core]
---
## Problem

Every new field type added a variant to `ncdb_core::basic::Kind` and `ncdb_core::model_lock::Kind`, so each one was a breaking change of `ncdb-core`, and Rust code that matched on these enums stopped compiling.

## Use

Both enums are `#[non_exhaustive]`. A new field type is now an additive change of `ncdb-core`, as a new field is in a schema.

```rust
match kind {
    Kind::Date => "date",
    Kind::Timestamp => "timestamp",
    _ => "other",
}
```

## Migration

A `match` on `basic::Kind` or `model_lock::Kind` outside `ncdb-core` adds a wildcard arm (`_ => …`). This is the last breaking change of this kind.
