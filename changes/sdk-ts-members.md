---
type: feature
tickets: [NOC-176]
components: [sdk-ts]
---
## Problem

An app in the browser could create spaces but could not share them: it had no invitation to give, and no way to add, list, re-role or remove the members of a space.

## Use

Give an admin this member's invitation, and add its member to a space. An account new to the admin's device fails with `unknownAccount`, carrying the fingerprint a person compares on both screens; then add it with that fingerprint:

```ts
const invitation = await db.invitation(); // on the joining member
send(String(invitation));

const project = await db.spaces.get(id); // on an admin
try {
  await project.members.add(nc.invitation(text), { role: "writer" });
} catch (error) {
  if (error.code !== "unknownAccount") throw error;
  if (await askPersonToCompare(error.fingerprint)) {
    const { notice } = await project.members.add(nc.invitation(text), {
      role: "writer",
      fingerprint: error.fingerprint,
    });
  }
}

const members = await project.members.list(); // [{ id, account, kind, role, self }]
await project.members.setRole(members[1].id, "reader");
await project.members.remove(members[1].id);
```
