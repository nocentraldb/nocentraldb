---
type: breaking
tickets: [NOC-458]
components: [core, sdk-ts, cli]
---
## Problem

A schema could not declare a rich-text field: `ncdb model build` refused `nc.richText()` as reserved, so an app could not prepare its model for rich text (RFC 0013).

## Use

`nc.richText()` declares a rich-text field. It takes `describe` and `deprecated`, no options, and cannot be personal or indexed.

```ts
export const model = nc.model({
  notes: nc.collection({
    title: nc.string(),
    body: nc.richText(),
  }),
});
```

`ncdb model build` writes it into the lock as `{ "type": "ncdb/richText", "rule": 1, "since": <version> }`, and adding it to an existing collection is an additive change. A device opens a database with such a lock. This release declares the field only: the field reads as unset, and its operations are kept, parked, until a later release folds them.

## Migration

Apps need no change.

`nc.richText()` returns a field with `describe` and `deprecated`, which reads as a string, instead of a `Field<never>`; code that relied on that type changes with it.

Rust code that matches on `ncdb_core::model_lock::Kind` needs a new arm: the enum gains the variant `RichText`, and it is not `#[non_exhaustive]`, so an exhaustive `match` stops compiling until it handles it or gets a wildcard arm.
