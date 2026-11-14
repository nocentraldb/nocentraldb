---
type: feature
tickets: [NOC-315]
components: [sdk-ts]
---
## Problem

An app in the browser could not tell its user what they would lose if the browser deleted the copy: it did not know which changes the relay had not confirmed, or whether the browser kept the copy. Nothing asked the browser for persistent storage, and the app could not see whether the view was being rebuilt.

## Use

The SDK asks the browser for persistent storage on open, without waiting. Follow the durability, or ask yourself after your own onboarding text:

```ts
const db = await nc.open({ model, lock, persistentStorage: "on-request" });
db.durability.subscribe(({ storage, unconfirmed, oldestUnconfirmedAt, atRisk }) => {
  if (atRisk) warn(`${unconfirmed} changes exist only on this device`);
});
const granted = await db.durability.requestPersistent(); // the browser's answer

db.view.current(); // { rebuild: { folded, total } | null }
```
