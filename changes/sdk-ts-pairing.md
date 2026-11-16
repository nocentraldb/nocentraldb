---
type: feature
tickets: [NOC-316]
components: [sdk-ts]
---
## Problem

A person's second device in the browser joined spaces as an account of its own: an app could not pair it into the person's account, so every admin had to confirm each device's fingerprint again.

## Use

On the new device, open for pairing and show the invitation; on a device of the account, pair it after the person confirmed; back on the new device, complete:

```ts
// New device:
const fresh = await nc.open({ model, lock, pairing: "request" });
show(String(await fresh.invitation()));

// A device of the account:
const { answer, added, askAdmins } = await db.account.pair({ kind: "device", invitation: nc.invitation(text) });
show(String(answer)); // added: [{ space, notice }], askAdmins: spaces whose admins to ask

// New device:
const { notices } = await fresh.account.completePairing(nc.pairingAnswer(answerText)); // devicePaired
```
