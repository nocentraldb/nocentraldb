# Getting started with the TypeScript SDK in the browser

This guide builds a small task list on NoCentralDB's TypeScript SDK, the package `@nocentraldb/sdk`, in a web page. You start with one database in one browser. Then you open the page in a second tab, sync through a relay that cannot read your data, and add a second member to a space.

NoCentralDB is not released yet. The SDK works in the browser, and this guide uses only what is merged, but names and formats can still change. Every TypeScript snippet below is type-checked against the SDK in the repository's gates, so the guide cannot drift from the SDK's API.

For a finished page that does the first half of this, see [apps/tasks-ts](../apps/tasks-ts/README.md). [RFC 0009](rfc/0009-sdk-api.md) describes every call. The Rust SDK has [a guide of its own](getting-started.md).

## What you need

- Rust, as `rust-toolchain.toml` pins it, to compile the SDK's core to WebAssembly. `rustup` installs it on the first `cargo` call.
- Node.js 24 or later and pnpm.
- A browser with the origin private file system (OPFS), Web Locks and `BroadcastChannel`. Current Chromium, Firefox and Safari have them; the repository tests in Chromium.
- A clone of this repository. Until the SDK is published to npm, you build your app inside it, in a folder under `apps/`, as [apps/tasks-ts](../apps/tasks-ts/README.md) does: the SDK and the schema build come from the clone. Run every command below from the clone's root.

## 1. Make the app

Make a folder `apps/my-tasks` and take the build and the local server from the example app. They find the SDK and the app's files relative to their own folder:

```sh
mkdir -p apps/my-tasks/src
cp apps/tasks-ts/build.mjs apps/tasks-ts/server.mjs \
  apps/tasks-ts/tsconfig.json apps/tasks-ts/tsconfig.build.json apps/my-tasks/
```

The clone's Cargo workspace takes every folder under `apps/` as a crate, so add the app to the `exclude` list of the root `Cargo.toml`, beside the example app, or every `cargo` command in the clone fails:

```toml
exclude = ["apps/tasks-ts", "apps/my-tasks"]
```

`build.mjs` builds the SDK, its core compiled to WebAssembly included, and then compiles `model.ts` and `src/` with `tsc` into `dist/`. `server.mjs` serves the page on `http://localhost:8336/`. A browser grants OPFS only to a secure context, and `localhost` is one.

The app's `package.json` takes the SDK from the clone:

```json
{
  "name": "my-tasks",
  "private": true,
  "type": "module",
  "dependencies": { "@nocentraldb/sdk": "workspace:*" }
}
```

Its `index.html` maps the package's name to the SDK's build output, which the server serves under `/sdk/`, so the page needs no bundler:

```html
<!doctype html>
<html lang="en">
<meta charset="utf-8">
<title>My tasks</title>
<script type="importmap">
  { "imports": { "@nocentraldb/sdk": "/sdk/index.js" } }
</script>
<script type="module" src="/dist/src/app.js"></script>
<body>
  <ul id="open"></ul>
</body>
</html>
```

With a bundler such as Vite, you import `@nocentraldb/sdk` as any package instead. The SDK finds its worker and its WebAssembly module next to its own module, so the app passes no URL for them.

## 2. Write the model and build its lock

You write the schema once, in TypeScript, in `apps/my-tasks/model.ts`. This one has one collection, `tasks`, with a title and a status, and one index, `byStatus`, that reads the tasks of one status in title order:

```ts model.ts
import { nc } from "@nocentraldb/sdk";

export const model = nc.model({
  tasks: nc.collection(
    {
      title: nc.string(),
      status: nc.oneOf(["open", "done"]),
    },
    {
      indexes: {
        byStatus: nc.index(["status", "title"], { equal: 1 }),
      },
    },
  ),
});
```

The model gives your code its types and its handles, such as `model.tasks`. The rules the database keeps come from `model.lock.json`, which `ncdb model build` writes from the model. Install the packages and build the SDK's TypeScript once, then build the lock in the app's folder:

```sh
pnpm install
node packages/sdk/build.mjs --ts-only
cd apps/my-tasks && cargo run -q -p ncdb -- model build && cd ../..
```

Commit `model.lock.json` with your code. Build it again after every change to the schema. The build refuses a change that browsers on the old lock could not keep, such as renaming a field.

Each program below is a version of `apps/my-tasks/src/app.ts`. Build it and serve the page:

```sh
node apps/my-tasks/build.mjs
node apps/my-tasks/server.mjs
```

After the first build, `node apps/my-tasks/build.mjs --app-only` compiles only the app again.

## 3. Open a database

`nc.open` takes the model and its lock. The page fetches the lock, which the server serves as `/model.lock.json`:

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({ model, lock, name: "tasks" });

const spaces = await db.spaces.list();
console.log(`spaces: ${spaces.length}`);

addEventListener("pagehide", () => {
  db.close().catch(() => {});
});
```

The SDK runs the core in a worker it starts itself. The database lives in this browser's OPFS, under the origin of the page and the name you give: 1 to 64 characters of `a`–`z`, `0`–`9`, `-` and `_`, or `"default"` when you leave it out. Its keys never reach your code.

The database folds and checks every change by the lock. So build the lock again after every change to `model.ts`: the browser SDK does not yet compare the model with the lock, as RFC 0009's `lockOutOfDate` will.

Every call runs in the browser. A read never waits for the network.

## 4. Create a space and write

Objects live in a space. A space is what you share: its members read and write it, and nobody else can. A write is one change in one space, signed and encrypted, and resolves when it is saved in the browser.

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({ model, lock, name: "tasks" });

const project = await db.spaces.create();
const written = await project.write((tx) => {
  tx.create(model.tasks, { title: "Write the README", status: "open" });
  tx.create(model.tasks, { title: "Buy milk", status: "open" });
});
console.log(`space ${project.id}, tasks ${written.ids.join(", ")}`);

// One create, update or delete is a write of its own.
const [task] = written.ids;
if (task !== undefined) {
  await project.update(model.tasks, task, { status: "done" });
}
```

Both creates in the `write` are one change: other members receive both or neither. The values are typed by the model: `status` takes `"open"` or `"done"`, and a field the model does not name is a type error.

A later page load finds the space in `db.spaces.list()` and opens it with `db.spaces.get(id)`.

## 5. Read an object by its id

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({ model, lock, name: "tasks" });

const params = new URLSearchParams(location.search);
const project = await db.spaces.get(params.get("space") ?? "");
const task = await project.get(model.tasks, params.get("task") ?? "");
if (task !== undefined) {
  console.log(`${task.title ?? ""} (deleted: ${task.$deleted})`);
}
```

A field you never wrote reads as `undefined`. A deleted object is still read by its id, with `$deleted` set, so you can show it or restore it with `project.restore(model.tasks, id)`.

## 6. Query through an index

A query reads a declared index: the rows with given values for its leading fields, in index order, a page at a time. There is no other query, so no read can scan a whole collection.

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({ model, lock, name: "tasks" });

const open = model.tasks.indexes.byStatus.equal("open").take(20);
let page = await db.find(open);
for (;;) {
  for (const task of page.items) {
    console.log(`[ ] ${task.title ?? ""}`);
  }
  if (!page.hasMore || page.cursor === undefined) break;
  page = await db.find(open.after(page.cursor));
}
```

`equal` takes as many values as the index's `equal` option allows, typed by the fields they fix: here one status. `db.find` reads every space of the database. An index declared with `scope: "space"` reads one space at a time, through `space.find`.

## 7. Follow a query

A subscription gives the query's page at once, and again after every write that touched the collection, whether it came from this tab, another tab or sync. It is how a page keeps a list on screen:

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({ model, lock, name: "tasks" });

const list = document.querySelector("#open");
const open = model.tasks.indexes.byStatus.equal("open").take(50);
const stop = db.subscribe(open, (page) => {
  list?.replaceChildren(
    ...page.items.map((task) => {
      const item = document.createElement("li");
      item.textContent = task.title ?? "";
      return item;
    }),
  );
});

// Call stop() when the list leaves the page.
addEventListener("pagehide", stop);
```

`space.subscribe(model.tasks, id, listener)` follows one object the same way, and `db.changes.subscribe(listener)` names the ids every write touched.

## 8. A second tab

Open the page in a second tab. Both tabs open the database of the same name, and the SDK makes them share it: one tab owns the database and runs its worker, and the other reaches it through that tab. Nothing in your code changes. A write in one tab shows in the other's subscriptions, and when the owning tab closes, another tab takes the database over without losing a write.

Every tab must open it with the same lock. A tab that opens it with another lock, such as an old tab after you deployed a new version of the page, fails with `modelRefused`; reloading that tab loads the new version.

So a page opens the database once per tab, by name, as above. One browser holds one database at a time per origin: while a tab has one open, a page of the same origin that opens another name fails. A second member, as in the next steps, is a second browser, or a second profile of the same browser.

## 9. Sync through a relay

The relay stores and forwards changes. It holds them encrypted and cannot read them. Each member opens its database with the relay's URL; the SDK syncs in the background, reconnects when the connection drops, and keeps working offline:

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const name = new URLSearchParams(location.search).get("name") ?? "tasks";
const db = await nc.open({
  model,
  lock,
  name,
  relay: "ws://localhost:7700",
});

const invitation = await db.invitation();
console.log(`invitation: ${invitation}`);
console.log(`fingerprint: ${invitation.fingerprint ?? ""}`);
```

In a page served over `https://`, the relay's URL is `wss://`; `ws://localhost` is for development.

The invitation is this member's public identity: its public keys and its account's fingerprint, and no secret, so it can travel any way you like. A relay lets only the accounts it knows create spaces. Open the page as `?name=laptop` in one browser and as `?name=phone` in another, or in another profile, and copy both invitations from the console. Then run the local relay from the clone with the invitation of the member that creates spaces, the laptop:

```sh
cargo run -q -p ncdb-relay-server --bin ncdb-relay -- \
  --database relay.sqlite --listen 127.0.0.1:7700 \
  --allow-invitation "ncdb-invite-1:…"
```

Reload both pages once the relay listens; the SDK also connects on its own, after a backoff.

## 10. Add a member to a space

An admin of a space adds a member by its invitation. The first time, the member's account is new to this database: `members.add` fails with `unknownAccount`, which carries the account's fingerprint. A person compares it with the fingerprint the new member shows, the way messaging apps compare safety numbers, and only then does the app pass it. The database remembers it, so a later member of that account needs no comparison.

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({
  model,
  lock,
  name: "laptop",
  relay: "ws://localhost:7700",
});

// The phone's invitation, as its page printed it.
const text = new URLSearchParams(location.search).get("invitation") ?? "";
const invitation = nc.invitation(text);
const project = await db.spaces.create();

try {
  const added = await project.members.add(invitation, { role: "writer" });
  console.log(added.notice.text);
} catch (error) {
  if (!(error instanceof nc.Error) || error.code !== "unknownAccount") {
    throw error;
  }
  // Show error.fingerprint, and add only after a person confirmed it
  // matches the fingerprint on the new member's screen.
  if (confirm(`Is the fingerprint ${error.fingerprint} on both screens?`)) {
    const added = await project.members.add(invitation, {
      role: "writer",
      fingerprint: error.fingerprint,
    });
    console.log(added.notice.text);
  }
}

// A write resolves in the browser; this waits until the relay holds it.
const written = await project.create(model.tasks, {
  title: "Shared with the phone",
  status: "open",
});
await db.sync.confirmed(written.change);
console.log("The relay holds the task.");
```

`members.add` returns a notice. Its text says what the call does not do: here, that the new member reads the whole history of the space. The new member receives the space at its next sync, and its subscriptions show the space's tasks as they arrive: the phone's page of step 7, opened with `name: "phone"` and the relay, lists "Shared with the phone".

`project.members.list()` lists the members, with the id that `members.setRole(id, role)` and `members.remove(id)` take. Roles are `"reader"`, `"writer"` and `"admin"`.

## 11. Show the sync status

`db.sync` reports the connection, how many of this member's changes the relay has not confirmed, and the last error that stops syncing. A page follows it to show whether its data has left the browser:

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({
  model,
  lock,
  name: "laptop",
  relay: "ws://localhost:7700",
});

const shown = document.createElement("p");
document.body.append(shown);
db.sync.subscribe((sync) => {
  if (sync.error !== undefined) {
    shown.textContent = `Not syncing: ${sync.error}`;
  } else if (sync.refused > 0) {
    shown.textContent = `The relay refused ${sync.refused} changes`;
  } else if (sync.connection === "connected") {
    shown.textContent =
      sync.unconfirmed === 0
        ? "Saved on the relay"
        : `${sync.unconfirmed} changes not on the relay yet`;
  } else {
    shown.textContent = `Relay: ${sync.connection}`;
  }
});

// The app may stay offline on purpose, and sync again later.
await db.sync.pause();
await db.sync.resume();
```

The relay refuses the changes of a space whose creator's account it does not allow, and `refused` counts them: the database `tasks` of the first steps, opened with the relay above, shows its spaces refused. They stay in the browser, and the relay never shares them.

`connection` is `"connected"`, `"connecting"`, `"offline"` (no relay reachable, and the SDK retries), `"paused"`, or `"none"` without a relay. In a tab that reaches another tab's database, it is the owning tab's connection.

`project.status.current()` and `project.status.subscribe(listener)` report what a space holds that a read does not show, such as changes that wait for a newer version of the app, or the relay's refusal of one of your changes.

## What the browser SDK does not do yet

RFC 0009 names more than the browser SDK has today. This guide leaves out what it lacks:

- **Accounts and devices.** A person's second device joins the space as a member of its own account, as in step 10. Pairing a device into one account, and the account's recovery code, are in the Rust SDK ([its guide](getting-started.md), steps 9 and 10) but not yet in the TypeScript SDK.
- **Node.js.** `nc.open` runs in a browser. The options `path`, `keyFile`, `inMemory` and `agent` of RFC 0009 are not there yet.
- **`space.inspect` and `db.view`,** the details of a conflict and the progress of a view rebuild.
- **The check of the model against the lock** in `nc.open`, which fails with `lockOutOfDate`. Until it comes, a model whose lock was not built again gives types the database does not keep.

## Where to go next

- [apps/tasks-ts](../apps/tasks-ts/README.md) is a finished page with the model above, with a test that drives it in Chromium.
- [RFC 0009](rfc/0009-sdk-api.md) describes every call, error and report.
- [Browser apps and durability](browser-durability.md) says what a browser copy keeps, what is lost when, and what to tell users.
- [The FAQ](faq.md) answers what the relay sees, how conflicts merge and how the schema changes.
