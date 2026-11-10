---
type: breaking
tickets: [NOC-80]
components: [sdk-ts, core]
---
## Problem
To type what an app reads (`nc.Infer`) and to deprecate a collection, the schema functions and the lock model had to carry more than they did. A field now knows what it reads as. A collection knows its fields and whether it is deprecated. A collection's deprecation in the Rust core holds a reason as well as a version.

## Use
In TypeScript, `nc.string()`, `nc.int()` and the other field functions return a field typed by what it reads as, and `nc.collection(...)` returns a collection typed by its fields. `nc.model(...)` returns a type without the collections marked `.deprecated(reason)`. In Rust, `model_lock::Collection::deprecated` is an `Option<Deprecation>`, like a field's, and `DraftCollection` has a `deprecated` reason.

```ts
const model = nc.model({
  teams: nc.collection({ name: nc.string() }).deprecated("Use groups."),
  tasks: nc.collection({ title: nc.string() }),
});
model.tasks; // typed
model.teams; // a type error: the collection is deprecated
```

## Migration
- TypeScript: code that names `model.<collection>` of a collection it has now deprecated no longer compiles; remove that use, which the deprecation intends. `nc.set(...)` no longer takes a deprecated field as its element, which `ncdb model build` already refused. Code that only declares schemas needs no change.
- Rust: read a collection's deprecation version as `collection.deprecated.as_ref().map(|d| d.since)` instead of `collection.deprecated`, and its reason as `d.reason`. Construct `Collection` and `DraftCollection` with the new values (`deprecated: None` keeps the old meaning).
