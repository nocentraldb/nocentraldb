---
type: feature
tickets: [NOC-325, NOC-326]
components: [sdk-rust]
---
## Problem
An app could not erase a person's personal fields: the SDK never wrote a shred, and a provisional erasure in a space of one account could not be taken back.

## Use
Name the person by one of their objects, have a device of a second account sign the request off, and erase in every space of the database:

```rust
let request = db.erasure_request(&anna).await?;          // text: ncdb-erase-1:…
let signed = other_device.account().sign_off(&request).await?; // text: ncdb-sign-off-1:…
let erased = db.erase(&request, &signed.sign_offs).await?;
for shred in &erased.shreds {
    println!("{} erased (provisional: {})", shred.space, shred.provisional);
}
```

In a space whose members all belong to one account, a device of that account signs off, and the erasure is provisional for 7 days; `space.erasures().take_back(shred)` takes it back within them.
