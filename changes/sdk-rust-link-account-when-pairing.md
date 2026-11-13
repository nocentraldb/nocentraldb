---
type: feature
tickets: [NOC-267, NOC-291]
components: [sdk-rust]
---
## Problem
A database opened with an account of its own, such as one an app created before the person had an account, could not be paired into the person's account: pairing refused its invitation.

## Use
Pair it as any device. The answer links its account to the person's, and the device gets an `AccountLinked` notice beside `DevicePaired`:

```rust
let paired = laptop.account().pair(PairKind::Device, &tablet.invitation().await).await?;
let notices = tablet.account().complete_pairing(&paired.answer).await?;
// [DevicePaired, AccountLinked]
```

The tablet keeps the link statement until it reaches the account space, so the account's other devices see that its old account's memberships belong to the person's account. An agent that has an account of its own is not linked yet: pairing it fails with `NotAllowed`.
