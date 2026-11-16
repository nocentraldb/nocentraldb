---
type: breaking
tickets: [NOC-347]
components: [sdk-ts, sdk-rust]
---
## Problem

In a private window of Safari or Firefox, which has no OPFS, the SDK kept the database in the memory of the tab that owned it. When that tab closed, or was hidden and another tab of the app took the database over, the other tab started with an empty copy. The changes the relay had not confirmed were lost, and `db.durability` then reported none unconfirmed, so the app could not tell.

## Use

`nc.open` now fails with the new error code `storageUnavailable` where the browser offers no OPFS, before it stores a key. Catch it and tell the user that the app does not work in a private window; `docs/browser-durability.md` has a text for it. Chromium's private window has OPFS and still opens.

```ts
try {
  const db = await nc.open({ model, lock, name: "tasks" });
} catch (error) {
  if (error instanceof nc.Error && error.code === "storageUnavailable") {
    showPrivateWindowNotice();
  } else throw error;
}
```

## Migration

An app that ran in a Safari or Firefox private window with `db.durability` reporting `storage: "temporary"` now gets `storageUnavailable` from `nc.open` there instead. Replace a check for `"temporary"` in the browser with a catch of `storageUnavailable`. A browser no longer reports `"temporary"`.
