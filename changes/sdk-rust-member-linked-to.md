---
type: breaking
tickets: [NOC-269, NOC-307]
components: [sdk-rust]
---
## Problem
After a database with an account of its own was linked into the person's account, `members.list()` showed its old fingerprint as an account like any other, with nothing to tell the app that it is the person's.

## Use
`MemberInfo` has a new field, `linked_to`: the fingerprint of the account a member's old account is linked into, when this database knows the link from its own account's link statements:

```rust
for member in space.members().list().await? {
    match member.linked_to {
        Some(account) => show_linked(member.account, account),
        None => show(member.account),
    }
}
```

`account` stays the account the member's certificate names. Another person's links are not known yet, so their members have `linked_to: None`.

## Migration
`MemberInfo` is now `#[non_exhaustive]`: read its fields as before, but an app can no longer build a `MemberInfo` with a struct literal or match it without `..`.
