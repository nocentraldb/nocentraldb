---
type: feature
tickets: [NOC-469]
components: [sdk-ts, sdk-rust]
---
## Problem
An app with open signup had no way to let its new users create spaces on a relay that admits accounts by admission tokens.

## Use
Pass a function that fetches an admission from the app's backend, which signs it for the account's identity after it authenticated the user:

```ts
const db = await nc.open({
  model, lock, relay,
  admission: (identity) => fetch("/api/ncdb-admission", { method: "POST", body: identity }).then((r) => r.text()),
});
db.account.identity; // the recovery key in hexadecimal
```

In Rust, `OpenOptions::admission(|identity| async move { ... })` and `db.account().identity()`. When the relay refuses a new space, the SDK calls the function, presents the admission and pushes the space again, once.
