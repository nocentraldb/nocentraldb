---
type: feature
tickets: [NOC-176]
components: [sdk-ts, sdk-rust, core]
---
## Problem

An app or an agent in the browser could follow a query or one object, but could not learn which objects changed, whether by its own write or through sync, to wake up or to update a cache of its own.

## Use

Subscribe to the change event; after each fold it gets the space, the collection and the ids the fold touched, and whether the change was this member's own write. It carries ids, never values, so the listener reads the objects itself:

```ts
const stop = db.changes.subscribe(({ space, collection, ids, origin }) => {
  // origin is "local" for this member's writes and "remote" for what sync brought
});
stop();
```
