---
type: feature
tickets: [NOC-316]
components: [sdk-ts]
---
## Problem

When a person lost a device, an app in the browser could not remove it from the person's spaces: the lost device kept the account key and stayed a member everywhere.

## Use

On another device of the account, with the recovery code the person types:

```ts
const { removed, askAdmins, notice } = await db.account.removeMember(lostDeviceId, {
  recover: words,
});
// removed: [{ space, notice }] (memberRemoved), askAdmins: spaces whose admins to ask,
// notice: accountKeyRenewed
```
