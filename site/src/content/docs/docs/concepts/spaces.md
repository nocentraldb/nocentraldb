---

title: Spaces and members
description: The unit of sharing, membership and keys.
sidebar: { order: 1 }
---

A **space** is the unit of sharing, membership and keys. Every object belongs to exactly one space. Fine-grained rights come from many small spaces.

A **member** is a person's device, an agent or a sub-agent, each with its own key and a role: `reader`, `writer` or `admin`. A member joins with an **invitation**, its public identity and never a key, which an admin of the space adds. An agent gives a sub-agent an **ad-hoc space** with copies of only what its task needs, never its own keys.

An **account** groups one person's devices and agents. A new device of the account joins by **pairing**, and the account's **recovery code** opens a new device when every device is lost.

Removing a member rotates the space key, so the member cannot read new changes. It does not delete the copy the member already holds.
