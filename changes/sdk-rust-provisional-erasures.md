---
type: feature
tickets: [NOC-234]
components: [core, sdk-rust]
---
## Problem
In a space whose members all belong to one account, an erasure the performer's own account signed off is provisional for 7 days. A Rust app could not see such an erasure, and nothing finalized it, so the person key was never destroyed.

## Use
`space.status().current().provisional_erasures` lists each provisional erasure as a `ProvisionalErasure` with `change`, `age` (counted from when this device first held the shred), `can_undo`, `performer` and `notice`, whose code is `NoticeCode::ErasurePending`. Every device of the account sees it, and an app must show it: it is how the account's owner notices, on another device, an erasure by a stolen one.

Nothing to call for the finalize. After 7 days on its own clock, an admin device of the performer's account writes the erasure decision that finalizes the erasure. The relay accepts it no sooner than 7 days by its own clock; until then the SDK keeps it and pushes it again every hour. Once a member folds the finalize, it destroys the person key. There is no call yet to take an erasure back.
