---
type: feature
tickets: [NOC-494]
components: [sdk-rust, sdk-ts]
---
## Problem
Recovery restores the account, not its spaces: a space comes back only when another of its members adds the new device. An app could not tell its user that a space has no other member, so losing this device would lose the space for good.

## Use
A space's status says when this member is the space's only member, and follows it as members are added or removed:

```ts
project.status.subscribe(({ onlyMember }) => {
  if (onlyMember) warn("Only this device holds this space. Losing it loses the space.");
});
```

In Rust it is `space.status().current().only_member`. An app that must not lose a space adds a second member: another person, an agent, or a second device of the account.
