---
type: feature
tickets: [NOC-80]
components: [sdk-ts, cli, core]
---
## Problem
An app could not type the objects it reads from its schema, and a schema could not deprecate a collection: `.deprecated()` worked only on a field, so `ncdb model build` could carry a deprecated collection over from an old lock but never set one.

## Use
`nc.Infer` types an object of a collection as the app reads it: every field optional, a choice as its options or `nc.Unknown`, an `nc.int` as a `number`, a set as an array, a link as an object id. `.deprecated(reason)` now chains on `nc.collection(...)` too; the collection leaves the model's type, and `ncdb model build` writes `deprecated` and `deprecatedReason` into the lock.

```ts
export const model = nc.model({
  teams: nc.collection({ name: nc.string() }).deprecated("Use groups."),
  tasks: nc.collection({ title: nc.string(), priority: nc.int() }),
});
export type Task = nc.Infer<typeof model.tasks>; // { title?: string; priority?: number }
```
