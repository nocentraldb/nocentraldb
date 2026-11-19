---

title: How NoCentralDB works
description: The core concepts on one example, a project app with projects, tasks and people, and the decisions a developer makes on the way.
---

NoCentralDB is a local-first database. Every member works on its own full copy, and a relay in the middle passes encrypted changes on without being able to read them. This page walks through the concepts on one example, and shows at each step the decision you make and what follows from it. Each section links its concept page or its RFC for depth.

The example is a project app. Every project has the same structure: a name and its tasks. A task names the person it is assigned to, and the app keeps the people, with their names and emails, apart from the projects.

The TypeScript snippets below are type-checked against the SDK, and the Rust ones compiled, so they hold for 0.1.

## The model: one for every space

You write the model once, in TypeScript, and it holds for every space of the app: every project space has the same collections, and so does the people space. It lists the collections, the fields with their field types, and the indexes every query runs through:

```ts model.ts
import { nc } from "@nocentraldb/sdk";

export const model = nc.model({
  // In the people space: one object per person, about that person.
  people: nc.collection({
    name: nc.string().personal({ person: "self" }),
    email: nc.string().personal({ person: "self" }),
  }),
  // In each project's space: the project and its tasks.
  projects: nc.collection({
    name: nc.string(),
  }),
  tasks: nc.collection(
    {
      title: nc.string(),
      status: nc.oneOf(["open", "done"]),
      project: nc.link("projects"),
      assignee: nc.ref("people"),
    },
    {
      indexes: {
        // The tasks of one project, by status: a query reads one space.
        byStatus: nc.index(["status", "title"], { equal: 1, scope: "space" }),
        // Every task of one person, in every project this member holds.
        byAssignee: nc.index(["assignee"], { equal: 1 }),
      },
    },
  ),
});
```

`ncdb model build` turns the model into `model.lock.json`, which the core and both SDKs read; the Rust SDK reads the same lock. Model changes are additive: the build refuses a change that clients on the previous lock could not keep, so a member that stays offline for weeks keeps working. More in [RFC 0005, the schema and model.lock.json](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0005-schema-and-model-lock.md).

## A database on every member

A member is a person's device, an agent or a sub-agent. On every member, your app runs on an SDK, the TypeScript SDK or the Rust SDK, over the same Rust core. The core keeps the encrypted log of every space the member belongs to and folds it, change by change, into a local SQLite view. Your app reads only from that view, through the indexes your model declares, so a read never waits for the network.

![One member, from top to bottom: your app or agent; the SDK, TypeScript or Rust, which manages the keys; and the core, ncdb-core, which folds the encrypted log of every space the member belongs to into a local SQLite view. Beside the core sit the field types, as plugins, and model.lock.json, which the core and the SDKs read.](/diagrams/0001-one-member.svg)

More in [Copies and convergence](/docs/concepts/convergence/) and [RFC 0006, the local view](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0006-local-view.md).

## Spaces: what goes where

A space is the unit of sharing, membership and keys, and every object belongs to exactly one space. **The decision: a space is the sharing boundary.** Whoever is a member of a space reads everything in it, so you put together what the same members may read. There are no rights per object or per field; fine-grained rights come from many small spaces.

In the example, every project is a space of its own, so a project's tasks reach only that project's members. The people live in one space of their own, because their names and emails are not for every project, and a person is named by tasks in many projects.

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({ model, lock, name: "projects" });

// One space for the people, and one per project.
const people = await db.spaces.create();
const apollo = await db.spaces.create();

const [anna] = (
  await people.create(model.people, {
    name: "Anna",
    email: "anna@example.com",
  })
).ids;
const [project] = (await apollo.create(model.projects, { name: "Apollo" }))
  .ids;
if (anna !== undefined && project !== undefined) {
  await apollo.create(model.tasks, {
    title: "Write the README",
    status: "open",
    project, // a link: an object in this space
    assignee: { space: people.id, id: anna }, // a reference: another space
  });
}
```

In Rust:

```rust no_run
use nocentraldb::{Db, OpenOptions, Reference, Value};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lock = std::fs::read_to_string("model.lock.json")?;
    let db = Db::open(OpenOptions::new(&lock).path("data")).await?;

    let people = db.spaces().create().await?;
    let apollo = db.spaces().create().await?;

    let person = [("name", Value::from("Anna")), ("email", "anna@example.com".into())];
    let anna = people.create("people", person).await?.ids[0];
    let project = apollo.create("projects", [("name", Value::from("Apollo"))]).await?.ids[0];
    apollo
        .create(
            "tasks",
            [
                ("title", Value::from("Write the README")),
                ("status", "open".into()),
                ("project", project.into()),
                ("assignee", Value::from(Reference::new(people.id(), anna))),
            ],
        )
        .await?;
    Ok(())
}
```

![Two spaces, Project A and HR, with their logs on the relay. Bob's phone writes a change in Project A. Agent X, a cloud function and Alice's laptop, members of Project A, decrypt it and fold it. Carol, a member of HR only, receives nothing from Project A and holds no key to read it.](/diagrams/0001-two-spaces.svg)

More in [Spaces and members](/docs/concepts/spaces/).

## Connecting objects: a link or a reference

**The decision: is the target in the same space?** A link, `nc.link`, points to an object in the same space; the SDK refuses a link into another space. A task's `project` is a link, since the project and its tasks share a space. A reference, `nc.ref` (in Rust, `Reference { space, id }`), points into any space. A task's `assignee` is a reference, since the people live in their own space. Both merge as one value: the last writer wins.

What a reference does not do follows from the space being the boundary ([RFC 0004, "What this does not do"](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0004-merge-rules-basic-types.md#what-this-does-not-do)):

- **It gives no access.** A member of Apollo who is not in the people space holds no copy of it: the task names Anna, and the member cannot read her.
- **It keeps nothing alive.** Deleting Anna, or erasing her, does not touch the tasks that name her. There is no cascade across spaces.
- **Nothing follows it.** No index, computed field or view reads through a reference, and a subscription to a task does not run again when Anna changes. To show her, the app reads her, and subscribes to her, in the people space.

To show the assignee, the app reads the task in its space and the person in theirs. A space this member holds no copy of fails with `notFound`:

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({ model, lock, name: "projects" });

const params = new URLSearchParams(location.search);
const apollo = await db.spaces.get(params.get("space") ?? "");
const task = await apollo.get(model.tasks, params.get("task") ?? "");

let assignee = "(nobody)";
if (task?.assignee) {
  try {
    const people = await db.spaces.get(task.assignee.space);
    const person = await people.get(model.people, task.assignee.id);
    assignee =
      person === undefined || person.$deleted
        ? "(no longer here)"
        : (person.name ?? "(erased)");
  } catch (error) {
    if (!(error instanceof nc.Error) || error.code !== "notFound") throw error;
    assignee = "(not shared with you)";
  }
}
console.log(assignee);
```

In Rust:

```rust no_run
use nocentraldb::{Db, ErrorCode, ObjectId, OpenOptions, Reference, SpaceId};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lock = std::fs::read_to_string("model.lock.json")?;
    let db = Db::open(OpenOptions::new(&lock).path("data")).await?;
    let space: SpaceId = std::env::args().nth(1).ok_or("a space id")?.parse()?;
    let id: ObjectId = std::env::args().nth(2).ok_or("a task id")?.parse()?;

    let apollo = db.spaces().get(space).await?;
    let Some(task) = apollo.get("tasks", id).await? else { return Ok(()) };
    let Some(assignee) = task.get::<Reference>("assignee")? else { return Ok(()) };
    let shown = match db.spaces().get(assignee.space).await {
        Ok(people) => match people.get("people", assignee.id).await? {
            Some(person) if !person.deleted() => {
                person.get::<&str>("name")?.unwrap_or("(erased)").to_string()
            }
            _ => "(no longer here)".to_string(),
        },
        Err(error) if error.code() == ErrorCode::NotFound => "(not shared with you)".to_string(),
        Err(error) => return Err(error.into()),
    };
    println!("{shown}");
    Ok(())
}
```

0.1 has no call that reports a reference's state. RFC 0005 designs one, `space.inspect`, which would name the state as ok, not accessible or missing; until it ships, the app tells them apart as above. More in [RFC 0005, "References into another space"](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0005-schema-and-model-lock.md#references-into-another-space).

## Personal fields and erasing a person

**The decision: which fields are about a person.** A field marked `.personal({ person })` is encrypted with the key of the person it is about: the object itself, `"self"`, or the person a link of the same collection names. In the example, a person's `name` and `email` are about that person. A task holds nothing about Anna but the reference: a reference cannot be the person of a personal field, because a person in another space has no key in this one that an erasure could destroy.

Erasing Anna destroys her person key in the people space. Her `name` and `email` then read as unset on every member that accepts the erasure; the Rust SDK lists them in `object.erased()`. The tasks that name her keep their reference, and the audit trail of who changed what survives. In 0.1 the Rust SDK erases; the browser SDK reads the erasure audit. More in [Erasing a person](/docs/concepts/shredding/).

## Roles: who may do what, per space

**The decision: a role per member in each space.** A member is a `reader`, a `writer` or an `admin` of a space, and the role holds for everything in it. In a project space, the project's lead is an admin and adds the team, the team and their agents are writers, and a stakeholder who only follows the project is a reader. In the people space, whoever keeps the people up to date is an admin or a writer, and everyone who should see the names is a reader. A team member who is not in the people space sees each assignee as "not shared".

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({ model, lock, name: "projects" });
const params = new URLSearchParams(location.search);

// On an admin of Apollo: add a teammate who writes, and let them read names.
const invitation = nc.invitation(params.get("invitation") ?? "");
const apollo = await db.spaces.get(params.get("apollo") ?? "");
const people = await db.spaces.get(params.get("people") ?? "");
await apollo.members.add(invitation, { role: "writer" });
await people.members.add(invitation, { role: "reader" });
```

Removing a member rotates the space key, so the member cannot read new changes; it keeps the copy it already holds. More in [Spaces and members](/docs/concepts/spaces/) and [RFC 0008, membership](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0008-membership.md).

## A change, signed and encrypted, through the relay

A write is one change in one space. The member encrypts it with the space's content key, signs it with its member key, and folds it into its own view at once: the write resolves before the relay has it. The relay checks the signature and that the author may write, appends the change to the space's log and hands it out from a cursor. Every other member checks the signature and that the author was a member, decrypts the change and folds it. A member that is offline gets the change when it reconnects.

The relay holds no keys and cannot read content. It does see metadata: which key is in which space, when and how much each key writes, and who reads which space from where.

![One change from Bob's phone to Alice's laptop through the relay, in eight steps. The phone encrypts the change with Project A's content key, signs it with its member key, and folds it into its own view. The relay checks the signature and that the author may write, and appends the change to Project A's log. The laptop checks the signature and that the author was a member when it wrote the change, decrypts it and folds it.](/diagrams/0001-one-change.svg)

More in [The relay](/docs/concepts/relay/), [RFC 0002, the change and the log](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0002-change-and-log-format.md) and [RFC 0007, the relay](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0007-relay.md).

## The view converges

Each field type says how concurrent changes merge, so the same set of changes gives the same state on every member, in any order of arrival. A change that loses to one its author had not seen stays identifiable as such, the same way on every member. With no relay at all, in island mode, members keep working, and every copy converges when they reconnect.

More in [Copies and convergence](/docs/concepts/convergence/) and [RFC 0004, merge rules](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0004-merge-rules-basic-types.md).

## Keys, handled by the SDK

The SDK creates, stores and rotates every key: the member key that signs a member's changes, the space key, whose generations encrypt the content, the account key that certifies a person's members, and a person key per person per space that encrypts that person's personal fields. Your code never touches a key; the one secret it passes on is the account's recovery code, which the person keeps.

More in [RFC 0003, keys and cryptography](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0003-keys-and-cryptography.md).
