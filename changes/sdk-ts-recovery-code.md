---
type: feature
tickets: [NOC-316]
components: [sdk-ts]
---
## Problem

An app in the browser could not show its user the account's recovery code, so a person who lost every device lost the account. It could not open a database to be paired to an account, recover an account on a new device, or tell that an open had recovered one.

## Use

Show the recovery code until the person stored it, then confirm:

```ts
const code = await db.account.recoveryCode(); // null once confirmed
if (code) {
  show(code.words()); // twelve words; String(code) is "RecoveryCode(redacted)"
  await code.confirm();
}

// On a new device:
const db = await nc.open({ model, lock, recover: words });
const notice = await db.account.recovered(); // { code: "accountRecovered", ... }

// A database to be paired to an account:
const fresh = await nc.open({ model, lock, pairing: "request" });
```
