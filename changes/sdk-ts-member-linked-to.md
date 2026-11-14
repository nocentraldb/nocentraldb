---
type: feature
tickets: [NOC-269, NOC-307]
components: [sdk-ts]
---
## Problem
In TypeScript, `members.list()` showed a member whose old account is linked into the person's account as an account like any other.

## Use
Each member has `linkedTo`: the fingerprint of the account its `account` is linked into, when this database knows the link, or `null`:

```ts
for (const member of await project.members.list()) {
  show(member.account, member.linkedTo ?? undefined);
}
```

Another person's links are not known yet, so their members have `linkedTo: null`.
