---
type: feature
tickets: [NOC-568]
components: [core, sdk-rust, sdk-ts, cli]
---
## Problem

A relation was `nc.link` inside a space and `nc.ref` across spaces, so a model read differently with each cut of its spaces. A reference into its own space lost what a link gives: the statuses deleted and wrong collection, and naming the person of a personal field. A reference field took only `{ space, id }`.

## Use

Write `nc.ref` for a reference wherever its target lives. A reference into its own space resolves as a link does, with the statuses ok, deleted, missing, wrong collection and moved, the same on every member of the space; one into another space keeps ok, not accessible and missing.

```ts
tasks: nc.collection({
  assignee: nc.ref("people"),
  notesAboutAssignee: nc.string().personal({ person: "assignee" }),
}),
```

`ncdb model build` locks `assignee` as `ncdb/link`, since it names the person of a personal field, and keeps a field already locked as `ncdb/link` when the schema now writes `nc.ref` for it: the lock and the model hash do not change. Such a field reads as `{ space, id }` with the object's own space, and a write of a target in another space is refused with `invalidValue`, naming the field.

A reference field takes `{ space, id }`, an object the SDK read, or a bare object id, which names the written object's own space:

```ts
await space.update(model.tasks, task, { assignee: person.$id });
await space.update(model.tasks, task, { assignee: person });
```

In Rust, a reference field takes a `Reference` or an `ObjectId`, and a link field a `Reference` into its own space.
