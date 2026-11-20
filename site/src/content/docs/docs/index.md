---

title: Getting started
description: What NoCentralDB 0.1 does, and the two guides that build a first app on it.
---

:::note[Release 0.1 is a pre-release]
NoCentralDB 0.1 is the first planned release. Its alphas are out, and names and formats can still change before 1.0. This page says what 0.1 does and what it does not do yet.
:::

## What 0.1 does

- **The Rust SDK,** the crate `nocentraldb`: spaces, members and invitations; sync through a relay, and island mode; accounts, with pairing, a recovery code and removing a device; erasure of a person; ad-hoc spaces for sub-agents.
- **The TypeScript SDK in the browser,** the package `@nocentraldb/sdk`: the same calls, except starting an erasure. It runs in Chromium, Firefox and Safari, and several tabs of an app share one database.
- **The relay,** locally on SQLite (`ncdb-relay`) or serverless on AWS, with DynamoDB and S3.
- **`ncdb model build`,** which turns the model you write in TypeScript into `model.lock.json`.

## What it does not do yet

- **Node.js and native mobile apps.** The TypeScript SDK runs in a browser only; on a server or in an agent, use the Rust SDK.
- **Rich text and streams.** Their field types are reserved in the schema and wait for their RFC.
- **Work orders.** Clean-up and maintenance jobs that one member does and another member checks: a second device for a job that can be undone, a second account for one that cannot. The main one is compaction, which moves old history into a signed, encrypted archive so a space's log stays small. 0.1 runs none of them. Erasing a person, which also needs a second account's sign-off, works in 0.1 through its own path, in the Rust SDK ([Erasing a person](/docs/concepts/erasure/)).
- **Private windows.** A private window of Firefox or Safari has no origin private file system (OPFS), so `nc.open` refuses with `storageUnavailable`. In Chromium's private window the database is deleted when the window closes.
- **A frozen tab in Safari.** On a phone, Safari can freeze a hidden tab before it gives the database's storage back. The visible tab's writes then wait until that tab runs again, and `db.durability` reports `waitingForTab`.

## Start with a guide

In 0.1 you build your first app inside a clone of the repository: `ncdb`, which builds the lock, comes from the clone, and so does the Rust SDK until it is on crates.io. Each guide starts with one database and ends with members syncing through a relay:

- [Getting started with the Rust SDK](/docs/guides/rust/), with [apps/tasks-rust](https://github.com/nocentraldb/nocentraldb/blob/main/apps/tasks-rust/README.md) as the finished app.
- [Getting started in the browser](/docs/guides/browser/), with [apps/tasks-ts](https://github.com/nocentraldb/nocentraldb/blob/main/apps/tasks-ts/README.md) as the finished page.

The alphas of the TypeScript SDK are on npm:

```sh
npm install @nocentraldb/sdk@alpha
```

## A first look at the code

You describe the model once, in TypeScript:

```ts
import { nc } from "@nocentraldb/sdk";

export const model = nc.model({
  tasks: nc.collection(
    {
      title: nc.string(),
      status: nc.oneOf(["open", "in_progress", "done"]),
      priority: nc.int(),
    },
    {
      indexes: {
        byStatusPriority: nc.index(["status", "priority"], { equal: 1 }),
      },
    },
  ),
});
```

`ncdb model build` turns it into `model.lock.json`, which the core and both SDKs read. Then you open a database and write:

```ts
const db = await nc.open({ model, lock, relay: "wss://relay.example.com" });
const project = await db.spaces.create();

await project.write((tx) => {
  tx.create(model.tasks, {
    title: "Write the README",
    status: "open",
    priority: 1,
  });
});
```

A `write` resolves once the change is in the local log and the view, before the relay has it. Read on in [Concepts](/docs/concepts/spaces/), and see [Browser apps and durability](/docs/browser-durability/) for what a browser copy keeps.
