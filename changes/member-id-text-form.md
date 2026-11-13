---
type: feature
tickets: [NOC-298]
components: [sdk-rust]
---
## Problem

A member id could not be written down: `MemberId` had no text form, so an app could not store which member it shows, or name one to remove from another device.

## Use

A member id prints and parses as 64 lowercase hex digits, like the other ids:

```rust
let me = project.members().list().await?[0].id;
let text = me.to_string(); // 64 lowercase hex digits
let again: MemberId = text.parse()?;
project.members().set_role(again, Role::Writer).await?;
```
