---
type: feature
tickets: [NOC-70]
components: [core, sdk-ts, cli]
---
## Problem
A developer could describe a model only on paper: nothing turned a TypeScript schema into `model.lock.json`, and nothing refused a schema change that clients on an older model could not keep.

## Use
Declare the model with the schema functions of `@nocentraldb/sdk`, then build the lock with `ncdb model build` and check it in CI with `ncdb model check`. Both run the schema with Node.js 24 or later.

```ts
import { nc } from "@nocentraldb/sdk";

export const model = nc.model({
  tasks: nc.collection(
    {
      title: nc.string().describe("One line; what has to be done."),
      status: nc.oneOf(["open", "in_progress", "done"]),
      dependsOn: nc.links("tasks"),
    },
    { indexes: { byStatus: nc.index(["status"], { equal: 1 }) } },
  ),
});
```

```sh
ncdb model build            # reads ./model.ts, writes ./model.lock.json
ncdb model check            # fails if the lock is out of date or the change is not additive
```

A change that is not additive, such as removing or retyping a field, is refused with a message that says why and what to do instead, and nothing is written.
