---
type: feature
tickets: [NOC-323]
components: [sdk-ts]
---
## Problem

A developer who changed the TypeScript model and did not run `ncdb model build` got types the database did not keep: `nc.open` opened with the old lock, and writes to a new field had no rule in it.

## Use

`nc.open` compares the model with its lock and fails with `lockOutOfDate` when the model names a collection, field or index the lock does not have, or with another type or options:

```ts
try {
  const db = await nc.open({ model, lock });
} catch (error) {
  if (error instanceof nc.Error && error.code === "lockOutOfDate") {
    // "model.lock.json is out of date: it has no field tasks.done. Run ncdb model build."
  }
}
```
