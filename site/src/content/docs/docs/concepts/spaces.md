---

title: Spaces and members
description: The unit of sharing, membership and keys.
sidebar: { order: 1 }
---

A **space** is the unit of sharing, membership and keys. Every object belongs to exactly one space. Fine-grained rights come from many small spaces.

A **member** is a person's device, an agent or a sub-agent, each with its own key and a role: `reader`, `writer` or `admin`. An agent is designed to give a sub-agent a small space with only what its task needs, never its own keys.

Removing a member rotates the space key, so the relay sends it no new changes. It does not delete the copy the member already holds.
