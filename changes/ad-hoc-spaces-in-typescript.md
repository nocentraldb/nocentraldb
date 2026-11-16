---
type: feature
tickets: [NOC-318]
components: [sdk-ts]
---
## Problem

An app in the browser could not create an ad-hoc space, which the Rust SDK already could: a space with a fresh key holding copies of only the objects a sub-agent's task needs.

## Use

Pass the objects the SDK read to `db.spaces.createAdHoc`:

```ts
const task = await project.get(model.tasks, id);
const { space, notice, copies } = await db.spaces.createAdHoc({ copy: [task] });
const copy = copies[0].copy;
```

Each copy is a new object with a fresh id. A link to an object that is copied too points to that object's copy; any other link is cleared, and `clearedLinks` counts them. Personal and plugin fields are not copied. `space.sourceOf(copy)` and `space.copyOf(project.id, task.$id)` find the pairs again later, on this database only. An object the SDK did not read, such as a spread of one, is refused with `invalidValue`. Opening a database as a sub-agent comes with the SDK in Node.js.
