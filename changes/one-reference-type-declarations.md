---
type: breaking
tickets: [NOC-568]
components: [core, sdk-ts]
---
## Problem
One reference type changes three declarations. In the TypeScript SDK, the type of what a write takes for a reference field now also takes an object the SDK read and a bare object id, so the API report lists the `Write` type as changed. In `ncdb-core`, `Reference::resolve` takes the space of the object that holds the reference, and `reference::Status` gains `Deleted`, `WrongCollection` and `Moved`, which a reference into its own space returns.

## Use
Every existing TypeScript write compiles and behaves as before: a reference field still takes `{ space, id }` and `null`. A caller of `Reference::resolve` passes the space of the object that holds the reference; a reference into another space resolves as before.

## Migration
TypeScript: none. `ncdb-core`: pass the holding object's space to `Reference::resolve`, and handle the three new `reference::Status` variants in a `match`.
