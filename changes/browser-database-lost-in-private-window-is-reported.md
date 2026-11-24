---
type: feature
tickets: [NOC-445, NOC-446]
components: [sdk-ts]
---
## Problem
In a Chromium private window, the browser drops a site's OPFS after about ten minutes with no database open, and keeps IndexedDB. The next `nc.open` started an empty copy, and nothing told the app: without a relay, all of its data was gone in silence; with one, the changes the relay had not confirmed.

## Use
`nc.open` now fails with `storageLost` when this browser had the database open before and its files are gone. Tell the user, then open with `ifStorageLost: "open-empty"`: the database starts empty under the same member key, and fetches again what the relay holds.

```ts
const db = await nc.open({ model, lock }).catch((error) => {
  if (!(error instanceof nc.Error) || error.code !== "storageLost") throw error;
  showStorageLost(); // docs/browser-durability.md has a text for it
  return nc.open({ model, lock, ifStorageLost: "open-empty" });
});
```

Playwright's `browser.newContext()` is such a private window: a test that runs longer than ten minutes, or reopens a closed database much later, should use `launchPersistentContext`.
