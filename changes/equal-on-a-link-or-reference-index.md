---
type: fix
tickets: [NOC-423]
introduced: 0.1.0
components: [sdk-ts]
---
## Symptom
In the TypeScript SDK, `.equal()` on an index whose equality field is an `nc.link` returned no objects, and on an `nc.ref` it failed with `invalidValue`.

## When
Any query that fixes a link or a reference, such as "children of X" or "orders of this customer", in 0.1.0-beta.1. The Rust SDK was not affected.

## Cause
The worker passed the query's values on by their form alone: a link arrived as the target's id in text and never matched the link the index holds, and a reference `{ space, id }` was read as no field value at all.

## Fix
The worker reads each equality value by its index field's type in the lock, as it reads the field in a write: `.equal(project.$id)` finds the project's tasks, and `.equal({ space, id })` the objects that reference it.
