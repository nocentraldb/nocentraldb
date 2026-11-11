# Spaces and members

> The unit of sharing, membership and keys.

A **space** is the unit of sharing, membership and keys. Every object belongs to exactly one space. Fine-grained rights come from many small spaces.

A **member** is a person's device, an agent or a sub-agent, each with its own key. An agent can give a sub-agent a small space with only what its task needs, never its own keys.

Removing a member rotates the space key, so it cannot read what is written afterwards. It does not delete the copy the member already holds.