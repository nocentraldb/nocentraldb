# @nocentraldb/sdk

The TypeScript SDK of [NoCentralDB](https://nocentraldb.io), a local-first database for apps where people and agents work on the same data. Every member works on its own full copy, under its own keys, and the relay in the middle cannot read any of it.

NoCentralDB 0.1 is a pre-release: names and formats can still change before 1.0. In 0.1 this SDK runs in the browser, in Chromium, Firefox and Safari, with its database on the origin private file system (OPFS); in Node.js it comes after 0.1. On a server or in an agent, use the Rust SDK, `nocentraldb`. [Getting started](https://nocentraldb.io/docs/) says what 0.1 does and does not do yet.

## Install

```sh
npm install @nocentraldb/sdk@beta
```

`ncdb model build`, which turns your model into `model.lock.json`, runs from a clone of [the repository](https://github.com/nocentraldb/nocentraldb) in 0.1, as [the browser guide](https://nocentraldb.io/docs/guides/browser/) shows.

## A first look

You describe the model once, in TypeScript:

```ts
import { nc } from "@nocentraldb/sdk";

export const model = nc.model({
  tasks: nc.collection(
    {
      title: nc.string(),
      status: nc.oneOf(["open", "done"]),
    },
    {
      indexes: {
        byStatus: nc.index({ where: ["status"], orderBy: ["title"] }),
      },
    },
  ),
});
```

Then you open the database with the lock, write to a space and follow a query. A write resolves once the change is in the local view, before the relay has it, and a subscription answers again after every change that may alter its page:

```ts
const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({ model, lock, relay: "wss://relay.example.com" });

const space = await db.spaces.create();
await space.write((tx) => {
  tx.create(model.tasks, { title: "Write the README", status: "open" });
});

const open = model.tasks.indexes.byStatus.where("open").take(20);
const stop = db.subscribe(open, (page) => console.log(page.items));
```

## Read on

- [Getting started in the browser](https://nocentraldb.io/docs/guides/browser/) builds a task list that syncs through a relay.
- [The TypeScript API reference](https://nocentraldb.io/docs/api/reference/) lists every public declaration.
- [Browser apps and durability](https://nocentraldb.io/docs/browser-durability/) says what a browser copy keeps and what to tell your users.

## License

MIT or Apache-2.0, at your option.
