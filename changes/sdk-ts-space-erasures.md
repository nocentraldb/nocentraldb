---
type: feature
tickets: [NOC-315]
components: [sdk-ts]
---
## Problem

An app in the browser could not audit a space's erasures: it could not show which members confirmed that they deleted a person's key, which only saw the erasure, and which never answered.

## Use

Read the audit of a space's erasures; every member may, a reader too:

```ts
const erasures = await project.erasures.list();
// [{ shred, state: "final" | "provisional" | "takenBack",
//    performer: { id, account }, signOff,
//    members: [{ id, account, current, acknowledged: "deleted" | "seen" | "missing" }] }]
```
