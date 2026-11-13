---
type: feature
tickets: [NOC-268, NOC-236]
components: [sdk-rust]
---
## Problem
A person who lost a phone or a laptop had no way to take back what it could do. The lost device stayed a member of every space it was in. Its copy of the account key could go on certifying new members and signing off erasures in the person's name.

## Use
On another device of the account, call `remove_member` with the lost device's member id and the recovery code:

```rust
let removed = db.account().remove_member(lost, &words).await?;
for (space, notice) in &removed.removed { /* the device was removed here, and the space key rotated */ }
for space in &removed.ask_admins { /* ask this space's admins to remove the device */ }
show(removed.notice); // AccountKeyRenewed
```

It removes the lost device from every space this device administers and from the account space. It then draws a new account key with the next number, which reaches the account's other devices when they sync. Every device carries the new key's statement into its spaces, after which an add certified by the old account key is refused with `InvalidCertificate`. A recovery code that is not the account's removes nothing and is refused with `NotAllowed`.
