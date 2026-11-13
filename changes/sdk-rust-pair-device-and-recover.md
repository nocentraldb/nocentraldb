---
type: feature
tickets: [NOC-267]
components: [sdk-rust]
---
## Problem
A device paired into an account joined none of the account's spaces, and an account whose every device was lost could not come back on a new one.

## Use
Pairing a device now adds it to every space where the pairing device is an admin, and to the account space, with one `DeviceAdded` notice per space; the spaces it could not add the device to come back for the app to ask their admins:

```rust
let paired = laptop.account().pair(PairKind::Device, &invitation).await?;
for (space, notice) in &paired.added { show(space, notice); }
for space in &paired.ask_admins { ask_admins_of(space); }
```

A device of an account that has no account space yet, such as a guest that only joined others' spaces, creates it when it pairs a device.

On a new device, recover the account from its recovery code. The words may be typed in any letter case and spacing; words that are no code are `InvalidValue`, and nothing is created:

```rust
let db = Db::open(OpenOptions::new(LOCK).path("./data").recover(&words)).await?;
if let Some(notice) = db.account().recovered().await? { show(notice); }
```

The recovered device holds a new account key, keeps the account's fingerprint, and is a member of no space until other members add it.
