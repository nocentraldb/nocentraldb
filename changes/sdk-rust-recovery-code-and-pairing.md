---
type: feature
tickets: [NOC-267, NOC-285]
components: [sdk-rust, core]
---
## Problem
Each database drew an account of its own, with no recovery code and no way to make a second device or an agent part of the same account. A person's laptop, phone and agents were unrelated accounts.

## Use
The first open of a database creates its account and recovery code. Show the code until the person confirms they stored it; `Display` and `Debug` print `RecoveryCode(redacted)`:

```rust
if let Some(code) = db.account().recovery_code().await? {
    show(code.words());
    code.confirm().await?;
}
```

To add a device or an agent, open its database for pairing, carry its invitation to a device of the account, and carry the answer back:

```rust
let phone = Db::open(OpenOptions::new(LOCK).path("./data").pairing()).await?;
let invitation = phone.invitation().await;                       // text, or a QR code
let paired = laptop.account().pair(PairKind::Device, &invitation).await?;
let notices = phone.account().complete_pairing(&paired.answer).await?;
```

A device receives the account key; an agent, opened with `.pairing().agent()`, receives a certificate only. A database waiting to be paired creates no space, and `members.add` refuses its invitation with `InvalidCertificate`. A database from before accounts gets its recovery code on its next open. The account's fingerprint is now over its recovery key, so it stays the same when the account key changes.
