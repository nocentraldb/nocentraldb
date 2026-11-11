---
type: feature
tickets: [NOC-169]
components: [sdk-rust]
---
## Problem
An admin could add members to a space but could not see who was in it, remove a member, or change a member's role.

## Use
`space.members().list()` gives the current members, each a `MemberInfo` with its `id`, its account's `Fingerprint`, its `kind`, its `role` and `is_self`. Pass the `MemberId` to `remove` or `set_role`; each returns a `Notice` that says what the change does not do.

```rust
let members = space.members();
let dan = members.list().await?.into_iter().find(|m| !m.is_self).unwrap();
let lowered = members.set_role(dan.id, Role::Reader).await?;   // NoticeCode::RoleSet
let removed = members.remove(dan.id).await?;                   // NoticeCode::MemberRemoved
```

`remove` rotates the space key, so the removed member cannot read what is written afterwards; it keeps its copy of everything it received before. Giving a reader the right to write again rotates the key first. Only an admin removes members or raises a role; any member may lower its own role, except the space's last admin (`ErrorCode::LastAdmin`). A space that holds a person key cannot be rotated yet: `remove`, and a `set_role` that gives a reader the right to write, are refused there with `InvalidValue`.
