---
type: feature
tickets: [NOC-582, NOC-583]
components: [sdk-ts]
---
## Problem
RFC 0010 lets a person confirm a new device by comparing its full fingerprint on both screens. In TypeScript only the new device could show it, as `(await db.invitation()).fingerprint`. On the device of the account, `nc.invitation(text)` leaves `fingerprint` unset, so an app could not show the digits to compare before `db.account.pair`.

## Use
On the device of the account, ask the open database for the invitation's fingerprint and show it next to the one the new device shows:

```ts
const invitation = nc.invitation(text);
const digits = await db.account.pairingFingerprint(invitation);
// "77045 61455 55361 …": compared by a person, then
const { answer } = await db.account.pair({ kind: "device", invitation });
```

For a device waiting to be paired it is that device's member fingerprint; for a certified invitation it is its account's fingerprint. The digits come from the core, so the Rust SDK's `Invitation::fingerprint()` shows the same.
