---
type: feature
tickets: [NOC-406]
components: [sdk-ts]
---
## Problem
In the browser, the TypeScript SDK could list and audit erasures, but it could not erase a person: an app such as a web app's "delete account" had to leave erasure to a Rust device.

## Use
The calls are the Rust SDK's, in camel case. The performer asks for the request, a device of another account signs it off, and the performer erases with the sign-offs. The app carries both as text.

```ts
const request = await db.erasureRequest(anna); // one of the person's objects, as the SDK read it
// On a device of another account:
const { signOffs, notSigned } = await other.account.signOff(nc.erasureRequest(String(request)));
// Back on the performer:
const { shreds, skipped, notice } = await db.erase(request, nc.signOffs(String(signOffs)));
// A provisional erasure, in a space of one account, can be taken back within 7 days:
await space.erasures.takeBack(shreds[0].shred);
```

`notice` is `erased` and says what erasure does not reach: devices that never come back online, copies made outside the database, and unmarked free text.
