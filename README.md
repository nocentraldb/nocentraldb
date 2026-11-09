# NoCentralDB

A local-first database for apps where people and agents work on the same data. Everyone works on their own copy, under their own keys, and the server in the middle cannot read any of it.

> **NoCentralDB is a design today, not a product.** It is not released, and no package below is published yet. The install commands and the code on this page show the planned API; names and behaviour may still change. The design lives in the [RFCs](docs/rfc/README.md), and we want your feedback on it while changes are still cheap.

## What it is for

You are building an app where people and agents work on the same records: tickets, notes, customer records, shared documents. Usually every read goes through a server. The agent waits for the network on each step and hits rate limits, the server can read everything it stores, and erasing a person's data means searching the database, the backups, the logs and every copy an agent made. If you need queries on the server, analytics across all users' data, or one global truth enforced at write time, keep your central database. NoCentralDB is for the apps where a central database is the wrong place ([RFC 0001](docs/rfc/0001-nocentraldb.md)).

What it does:

- **Spaces for sharing.** A space is the unit of sharing, membership and keys, and every object belongs to exactly one space. A member is a person's device, an agent or a sub-agent, each with its own key. An agent can give a sub-agent a small space with only what its task needs ([RFC 0008](docs/rfc/0008-membership.md)).
- **A full copy on every member.** Each device and each agent keeps the encrypted log of every space it belongs to and folds it into a local SQLite view. Reads never wait for the network ([RFC 0006, "Reads never touch the network"](docs/rfc/0006-local-view.md#reads-never-touch-the-network)).
- **A relay that cannot read.** The relay checks signatures and membership, stores the encrypted changes and hands them out. It holds no keys, so it cannot read content ([RFC 0007](docs/rfc/0007-relay.md)).
- **Every copy converges.** The same changes give the same state on every member, in any order of arrival. Each field type says how concurrent edits merge, and an edit that loses to one its author had not seen stays visible as such ([RFC 0004](docs/rfc/0004-merge-rules-basic-types.md)).
- **Works offline.** With no relay, members keep working; when they reconnect, every copy converges ([RFC 0007, "Island mode"](docs/rfc/0007-relay.md#island-mode)).
- **One schema, written in TypeScript.** You describe the model once. `ncdb model build` turns it into a versioned `model.lock.json` that the Rust core and both SDKs read. Schema changes are additive, so a client that stays offline for weeks keeps working ([RFC 0005](docs/rfc/0005-schema-and-model-lock.md)).
- **Queries that cost only their result.** Queries run through declared indexes and return sorted pages with a cursor ([RFC 0006, "Declared indexes and ordered pages"](docs/rfc/0006-local-view.md#declared-indexes-and-ordered-pages)).
- **No keys in your code.** The SDK creates, stores and rotates every key. You work with spaces, members and invitations ([RFC 0003, "How the SDK hides keys"](docs/rfc/0003-keys-and-cryptography.md#how-the-sdk-hides-keys)).
- **Erasure that destroys a key.** The schema marks personal fields, and each is encrypted with one key per person per space. Erasing the person destroys those keys, which reaches archives and old snapshots too, while the audit trail of who changed what survives ([RFC 0003, "Shredding a person's key"](docs/rfc/0003-keys-and-cryptography.md#shredding-a-persons-key)).
- **One core on every platform.** The same Rust core runs natively on desktop and mobile and as WebAssembly in the browser ([RFC 0006, "SQLite on each target"](docs/rfc/0006-local-view.md#sqlite-on-each-target)).

What it does not do:

- **The relay sees metadata.** It cannot read content, but it sees which key is in which space, when and how much each key writes, and who reads which space from where. It can also withhold changes ([RFC 0002](docs/rfc/0002-change-and-log-format.md#what-the-relay-can-see-and-what-it-cannot), [RFC 0007, "What the relay sees"](docs/rfc/0007-relay.md#what-the-relay-sees)).
- **Erasure has limits.** It does not reach a device that never comes back online, a modified client that ignores it, a relay backup from before the erasure that a member obtains, exports and screenshots, or personal data in unmarked free text ([RFC 0001, "The team lead"](docs/rfc/0001-nocentraldb.md#the-team-lead-who-has-to-pass-the-data-protection-review)).
- **Removing a member does not delete its copy.** It keeps what it received before the removal ([RFC 0008, "What this does not do"](docs/rfc/0008-membership.md#what-this-does-not-do)).
- **Rights apply per space.** There are no row-level or field-level rights, no queries on the server and no full-text search in the core in v1 ([RFC 0001, "What it is not"](docs/rfc/0001-nocentraldb.md#what-it-is-not)).
- **Not post-quantum in v1.** Ciphertext recorded today could be read by a future quantum computer ([RFC 0003, decision 9](docs/rfc/0003-keys-and-cryptography.md#decisions-for-the-owner)).
- **A browser can delete its copy.** Under storage pressure, or in Safari after seven days without use, a browser may evict the local copy, with the changes the relay has not yet received ([RFC 0006, "The browser"](docs/rfc/0006-local-view.md#the-browser)).

The [use cases](docs/use-cases.md) show the design in practice, and the [FAQ](docs/faq.md) answers the questions readers ask most often.

## Install

Nothing is published yet. Once the first release is out, you will install:

- the TypeScript SDK, `@nocentraldb/sdk`, from npm, for web apps, Node and agents written in TypeScript:

  ```sh
  npm install @nocentraldb/sdk
  ```

- the Rust SDK, `nocentraldb`, from crates.io:

  ```sh
  cargo add nocentraldb
  ```

- the command-line tool `ncdb`, which builds `model.lock.json` from your schema. How it is distributed is not decided yet.

Watch the repository's releases to hear when the first one is out.

## Code examples

These examples show the planned API. The schema functions and the `ncdb` commands are decided in [RFC 0005, "Public names"](docs/rfc/0005-schema-and-model-lock.md#public-names). The calls that open a database, share a space and read a page are sketches from [RFC 0003](docs/rfc/0003-keys-and-cryptography.md#how-the-sdk-hides-keys) and [RFC 0001](docs/rfc/0001-nocentraldb.md#the-design-in-one-page); their final names come with the SDK's API review. No RFC names the call that writes an object yet, so it is left out.

### Describe the model

You describe the model once, as a TypeScript module ([RFC 0005, "The schema in TypeScript"](docs/rfc/0005-schema-and-model-lock.md#the-schema-in-typescript)):

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
      computed: { blocked: nc.reach("dependsOn", { status: ["open", "in_progress"] }) },
      views: { ready: nc.view("tasks", { where: { blocked: false }, sort: ["priority"] }) },
      indexes: {
        byStatusPriority: nc.index(["status", "priority"], { equal: 1 }),
      },
    },
  ),
});

export type Task = nc.Infer<typeof model.tasks>;
```

`.personal({ person: "self" })` marks a field as personal: erasing that person makes it unreadable ([RFC 0005, "Personal fields"](docs/rfc/0005-schema-and-model-lock.md#personal-fields)). `blocked` is a computed field and `ready` a view of the ready tasks in priority order; RFC 0005 reserves their places, and a later RFC designs them ([RFC 0005, "Computed fields and views"](docs/rfc/0005-schema-and-model-lock.md#computed-fields-and-views-named-not-designed)).

### Build model.lock.json

`ncdb model build` reads the schema and the previous lock, refuses a change that old clients could not read, and writes `model.lock.json`. `ncdb model check` runs the same check in CI and writes nothing ([RFC 0005, "The additive-change rule"](docs/rfc/0005-schema-and-model-lock.md#the-additive-change-rule)):

```sh
ncdb model build
ncdb model check
```

### Open a database and share a space

Opening the database creates this member's key; creating a space creates the space's key. Adding a member takes an invitation, never a key, and removing one returns a notice that says what removal does not do ([RFC 0003, "How the SDK hides keys"](docs/rfc/0003-keys-and-cryptography.md#how-the-sdk-hides-keys)). The roles reader, writer and admin are working names ([RFC 0008, "Roles and rights"](docs/rfc/0008-membership.md#roles-and-rights)):

```ts
const db = await nc.open({ model });
const project = await db.spaces.create({ name: "Project A" });

await project.members.add(invitation, { role: "writer" });
const removal = await project.members.remove(memberId);
removal.notice; // "The removed member keeps its copy of everything it received before now."
```

### Read a page

A read comes from the local view, in index order, 50 rows at a time, and the next page continues from a cursor ([RFC 0001, "The design in one page"](docs/rfc/0001-nocentraldb.md#the-design-in-one-page), [RFC 0006, "Ordered pages"](docs/rfc/0006-local-view.md#ordered-pages)):

```ts
const page = await db.find(model.tasks.views.ready.take(50));
const more = await db.find(model.tasks.views.ready.take(50).after(page.cursor));
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
