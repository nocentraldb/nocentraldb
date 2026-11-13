---
type: feature
tickets: [NOC-176]
components: [sdk-ts]
---
## Problem

An app in the browser could declare its model with `@nocentraldb/sdk`, but could not open a database, write or read: the TypeScript SDK had only the schema functions.

## Use

Open the member's database in the browser, create a space, write and read. The SDK starts its own worker, which runs the core from the WebAssembly module the package ships, and keeps the database on OPFS under its `name`:

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "./model";
import lock from "./model.lock.json";

const db = await nc.open({ model, lock, name: "default" });
const project = await db.spaces.create();
const { ids } = await project.write((tx) => {
  tx.create(model.tasks, { title: "Ship it", tags: nc.add("sdk") });
});
const task = await project.get(model.tasks, ids[0]);
await db.close();
```

`project.create`, `update`, `delete` and `restore` are one-call writes; `null` clears a field. Every error is an `nc.Error` with a `code`, such as `notFound`, `invalidValue` or `closed`.
