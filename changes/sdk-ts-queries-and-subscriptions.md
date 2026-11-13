---
type: breaking
tickets: [NOC-176]
components: [sdk-ts]
---
## Problem

An app in the browser could write and get objects by id, but could not list them through the indexes its model declares, page through them, or follow what changes.

## Use

Every declared index has a handle on its collection; `.equal` takes exactly the values of its equality prefix, checked by TypeScript. Page through it with `db.find`, or `space.find` for an index with `scope: "space"`, and follow a page or one object:

```ts
const query = model.tasks.indexes.byStatusPriority.equal("open").take(50);
const page = await db.find(query); // { items, cursor, hasMore }
const more = await db.find(query.after(page.cursor));

const stop = db.subscribe(query, (page) => render(page.items));
const stopOne = project.subscribe(model.tasks, taskId, (task) => show(task));
stop();
```

A subscription gets the whole page at once and again after every write or sync that touched its collection. A cursor stays on the device, prints as `[redacted]`, and another database refuses it with `cursorOfAnotherModel`.

## Migration

Calls compile unchanged: `nc.collection(fields, { indexes })` and `nc.index(fields, { equal })` take the same arguments. Their return types now carry the indexes and the equality prefix, so only code that annotates the old return types explicitly changes. Nothing of `@nocentraldb/sdk` is published yet.
