---
type: feature
tickets: [NOC-177]
components: [sdk-ts]
---
## Problem

A browser database could be open in one tab only: `nc.open` in a second tab of the same origin failed with `storageFailed`, because the browser lets one worker at a time hold its OPFS files. Users open apps in several tabs.

## Use

Open the database in every tab as usual. The first tab owns it, the others reach it through the owner, and subscriptions fire in every tab. When the owning tab closes or calls `db.close()`, another tab takes the database over, and every tab reads again what it follows:

```ts
const db = await nc.open({ model, lock, name: "default" }); // in any number of tabs
```

A write that was in flight while its owner tab went away fails with `storageFailed` and may or may not have run; read before you write it again. A tab whose `model.lock.json` differs from the owning tab's fails to open with `modelRefused`.
