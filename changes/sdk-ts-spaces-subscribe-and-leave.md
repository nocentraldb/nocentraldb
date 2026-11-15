---
type: feature
tickets: [NOC-315]
components: [sdk-ts]
---
## Problem

An app in the browser could list its spaces but not follow the list: it had to poll `db.spaces.list()` to learn that another device shared a space with this member. It could not tell a space of another model from its own, and it could not leave a space.

## Use

Follow the space list, and leave a space you no longer need:

```ts
const stop = db.spaces.subscribe((all) => showSpaces(all)); // [{ id, role, model: "ours" | "another" }]

const { notice } = await project.leave(); // notice.code === "spaceLeft"
// The space's last admin is refused with `lastAdmin`.
```
