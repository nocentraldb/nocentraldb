# NoCentralDB

A local-first database for apps where people and agents work on the same data. Everyone works on their own copy, under their own keys, and the server in the middle cannot read any of it.

> **NoCentralDB is not yet a product.** We are currently working on the design and strongly encourage you to share your feedback while changes are still cheap (see the [RFCs](docs/rfc/README.md)). The install commands and the code on this page give you an impression of what the product will look like once the current design is fully implemented.

## What it is for

You are building an app where people and agents work on the same records: tickets, notes, customer records, shared documents. With **NoCentralDB**, this data sits right next to where the work happens: on the user's computer or wherever the agent runs. This makes processing super fast and independent of whether a server is available. Collaboration runs through a relay server that transports the encrypted changes to the data; the relay has no way to see their content. This allows you to work even on the most sensitive data you can imagine.

What it does:

- **Spaces for sharing.** A space is the unit of sharing, membership and keys, and every object belongs to exactly one space. A member is a person's device, an agent or a sub-agent, each with its own key. An agent can give a sub-agent a small space with only what its task needs.
- **A full copy on every member.** Each device and each agent keeps the encrypted log of every space it belongs to and folds it into a local SQLite view. Reads never wait for the network.
- **A relay that cannot read.** The relay checks signatures and membership, stores the encrypted changes and hands them out. It holds no keys, so it cannot read content.
- **Every copy converges.** The same changes give the same state on every member, in any order of arrival. Each field type says how concurrent edits merge, and an edit that loses to one its author had not seen stays visible as such.
- **Works offline.** With no relay, members keep working; when they reconnect, every copy converges ("Island mode").
- **One schema, written in TypeScript.** You describe the model once. `ncdb model build` turns it into a versioned `model.lock.json` that the Rust core and both SDKs read. Schema changes are additive, so a client that stays offline for weeks keeps working.
- **Queries that cost only their result.** Queries run through declared indexes and return sorted pages with a cursor.
- **No keys in your code.** The SDK creates, stores and rotates every key. You work with spaces, members and invitations.
- **Erasure that destroys a key.** The schema marks personal fields, and each is encrypted with one key per person per space. Erasing the person destroys those keys, which reaches archives and old snapshots too, while the audit trail of who changed what survives.
- **One core on every platform.** The same Rust core runs natively on desktop and mobile and as WebAssembly in the browser.

Check out more [use cases](docs/use-cases.md) for **NoCentralDB** and read the [FAQ](docs/faq.md) for answers to the questions readers ask most often.

## Install

Once the first release is out, you will install:

- the TypeScript SDK, `@nocentraldb/sdk`, from npm, for web apps, Node and agents written in TypeScript:

  ```sh
  npm install @nocentraldb/sdk
  ```

- the Rust SDK, `nocentraldb`, from crates.io:

  ```sh
  cargo add nocentraldb
  ```

- the command-line tool `ncdb`, which builds `model.lock.json` from your schema. How it is distributed is not decided yet.

## Code examples

These examples show the planned API, as [RFC 0009](docs/rfc/0009-sdk-api.md) defines it.

### Describe the model

You describe the model once, as a TypeScript module:

```ts
import { nc } from "@nocentraldb/sdk";

export const model = nc.model({
  people: nc.collection({
    name: nc.string().personal({ person: "self" }),
    email: nc.string().personal({ person: "self" }),
  }),
  tasks: nc.collection(
    {
      title: nc.string().describe("One line; what has to be done."),
      status: nc.oneOf(["open", "in_progress", "done"]),
      priority: nc.int(),
      assignee: nc.link("people"),
      dependsOn: nc.links("tasks"),
    },
    {
      indexes: {
        byStatusPriority: nc.index(["status", "priority"], { equal: 1 }),
      },
    },
  ),
});

export type Task = nc.Infer<typeof model.tasks>;
```

`.personal({ person: "self" })` marks a field as personal: erasing that person makes it unreadable. `byStatusPriority` is an index: every query runs through an index the model declares.

### Build model.lock.json

`ncdb model build` reads the schema and the previous lock, refuses a change that old clients could not read, and writes `model.lock.json`. `ncdb model check` runs the same check in CI and writes nothing:

```sh
ncdb model build
ncdb model check
```

### Open a database

Opening the database checks the model against the lock and creates this member's key on first open. The relay is optional; without it, the database stays local:

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "./model";
import lock from "./model.lock.json";

const db = await nc.open({ model, lock, relay: "wss://relay.example.com" });
```

### Write

Objects live in a space, which the device creates on its own, offline if need be. One `write` is one signed, encrypted change: it resolves when the change is in the local log and the view, before the relay has it:

```ts
const project = await db.spaces.create();

await project.write((tx) => {
  const task = tx.create(model.tasks, { title: "Write the README", status: "open", priority: 1 });
  tx.update(model.tasks, task, { status: "in_progress" });
});
```

### Read and subscribe

A query names a declared index, fixes its leading fields and returns a page in index order, with a cursor for the next one. A subscription calls the listener with the page at once and again after every change that may alter it. Neither waits for the network:

```ts
const query = model.tasks.indexes.byStatusPriority.equal("open").take(50);
const page = await db.find(query);
const more = await db.find(query.after(page.cursor));

const stop = db.subscribe(query, (fresh) => render(fresh));
```

### Share a space

The joining member, a device or an agent, creates an invitation: its public identity, never a key. An admin of the space adds it with a role. Every call that changes who can read returns a notice, `{ code, text }`, that says what the change does not do:

```ts
// On the joining member:
const invitation = await db.invitation();
send(invitation.toString());

// On an admin of the space:
await project.members.add(nc.invitation(text), { role: "writer" });
const removal = await project.members.remove(memberId);
removal.notice; // text: "The removed member keeps its copy of everything it received before now."
```

### Sync status

The SDK syncs in the background. The app reads the connection and how many of its changes the relay has not confirmed yet:

```ts
db.sync.current(); // { connection: "connected", unconfirmed: 0, ... }
```

## Contributing

NoCentralDB is designed in the open. The contribution we want most right now is feedback on the RFCs: tell us about your app and where the design would break for you, as [Giving feedback](docs/rfc/README.md#giving-feedback) describes.

- [CONTRIBUTING.md](CONTRIBUTING.md) says what we ask for, from RFC feedback to bug reports and offers to take a roadmap milestone, and what a pull request needs.
- [docs/repository.md](docs/repository.md) explains the repository: its layout, the tools to install and the gates every change passes.
- Agents are welcome under the [AI policy](.github/AI_POLICY.md).
- Report vulnerabilities privately, as the [security policy](.github/SECURITY.md) describes.
- Everyone follows the [code of conduct](.github/CODE_OF_CONDUCT.md).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option. See [CONTRIBUTING.md](CONTRIBUTING.md).
