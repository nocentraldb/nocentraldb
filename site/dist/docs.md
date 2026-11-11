# Getting started

> Install the SDK, describe a model and open a database.

:::caution[Not yet a product]
NoCentralDB is in its design stage. The commands and the code on this page show the API that [RFC 0009](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0009-sdk-api.md) defines. None of it is released yet.
:::

## Install

The TypeScript SDK, for web apps, Node and agents written in TypeScript:

```sh
npm install @nocentraldb/sdk
```

The Rust SDK:

```sh
cargo add nocentraldb
```

The command-line tool `ncdb` builds `model.lock.json` from your schema. How it is distributed is not decided yet.

## Describe the model

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

Then build the lock that the Rust core and both SDKs read:

```sh
ncdb model build
```

## Open a database and write

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

A `write` resolves once the change is in the local log and the view, before the relay has it. Read on in [Concepts](/docs/concepts/spaces/).