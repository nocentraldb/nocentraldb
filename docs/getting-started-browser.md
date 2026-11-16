# Getting started with the TypeScript SDK in the browser

This guide builds a small task list on NoCentralDB's TypeScript SDK, the package `@nocentraldb/sdk`, in a web page. You start with one database in one browser. Then you open the page in a second tab, sync through a relay that cannot read your data, and add a second member to a space.

NoCentralDB is not released yet. The SDK works in the browser, and this guide uses only what is merged, but names and formats can still change. Every TypeScript snippet below is type-checked against the SDK in the repository's gates, so the guide cannot drift from the SDK's API.

For a finished page that does the first half of this, see [apps/tasks-ts](../apps/tasks-ts/README.md). [RFC 0009](rfc/0009-sdk-api.md) describes every call. The Rust SDK has [a guide of its own](getting-started.md).

## What you need

- Rust, as `rust-toolchain.toml` pins it, to compile the SDK's core to WebAssembly. `rustup` installs it on the first `cargo` call.
- Node.js 24 or later and pnpm.
- A browser with the origin private file system (OPFS), Web Locks and `BroadcastChannel`. Current Chromium, Firefox and Safari have them. The repository runs the SDK's browser tests in Playwright's builds of Chromium, Firefox and WebKit, in a normal window, and the owner handover in a private window too; Playwright's WebKit is not Safari, and no test is skipped in any of them.
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

The SDK runs the core in a dedicated worker it starts itself. Your code never sees a key.

The database folds and checks every change by the lock. So build the lock again after every change to `model.ts`. `nc.open` compares the model with the lock: when the model names a collection, field or index the lock does not have, or with another type or options, it fails with `lockOutOfDate`, and its message names what differs and says to run `ncdb model build`.

Every call runs in the browser. A read never waits for the network.

### Where the database lives

The database lives in the origin's private file system (OPFS), under the `name` you give: 1 to 64 characters of `a`–`z`, `0`–`9`, `-` and `_`, or `"default"` when you leave it out. Every database of the page's origin shares one pool there, and each name is a file of its own in it. Your app chooses only the name; the browser decides where on the disk OPFS is, and no path of yours reaches it ([RFC 0009, "Open and close"](rfc/0009-sdk-api.md#open-and-close); [RFC 0006, "The browser"](rfc/0006-local-view.md#the-browser)).

The keys are not in OPFS. A page reaches no platform keystore, so the SDK keeps them in IndexedDB, in a database of the same name, sealed under a non-extractable WebCrypto key: the page can use that key, but no code can read its bytes ([RFC 0003, "The member key"](rfc/0003-keys-and-cryptography.md#the-member-key)).

The browser may evict an origin's data when storage runs low, and Safari deletes a site's data after seven days without a visit unless it was added to the home screen. An eviction takes the database and its keys together, and with them every change the relay has not confirmed yet. `nc.open` asks the browser for persistent storage, unless you open with `persistentStorage` set to `"on-request"` and ask later; `db.durability` reports what is at risk, as step 12 shows ([RFC 0006, "Durability of a browser copy"](rfc/0006-local-view.md#durability-of-a-browser-copy)). A private window of Firefox or Safari has no OPFS: the database then lives in the memory of the tab that owns it (step 8), and `db.durability` says `"temporary"`. It lives only as long as that tab owns it, not until the window closes: when the owning tab closes, or is hidden and another tab of the window takes the database over, the other tab starts with an empty copy. Every change the relay had not confirmed is lost with it, and the copy syncs the rest from the relay again. Chromium has OPFS in a private window, so its copy outlives the owning tab.

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

When the owning tab is hidden, as when a person switches tabs on a phone, it gives the database up, and the visible tab takes it over, so a phone that freezes the hidden tab does not hold the visible tab's writes. Safari has a limit here, which the end of this step describes.

Every tab must open it with the same lock. A tab that opens it with another lock, such as an old tab after you deployed a new version of the page, fails with `modelRefused`; reloading that tab loads the new version.

So a page opens the database once per tab, by name, as above. One browser holds one database at a time per origin: while a tab has one open, a page of the same origin that opens another name fails. A second member, as in the next steps, is a second browser, or a second profile of the same browser.

### A frozen tab in Safari

In Safari a tab can give the database's storage back only while it still runs. A phone may freeze a hidden tab right after it gave the database up, before it gave the storage back. The visible tab then cannot open the database yet: its writes wait, and run once the frozen tab runs again. They are not lost, and they do not fail. In Chromium and Firefox the visible tab takes over while the hidden tab is frozen.

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

`db.sync` reports the connection, how many of this member's changes the relay has not confirmed, and the last error of syncing. A page follows it to show whether its data has left the browser:

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
## 12. Keep what the browser could lose

`db.durability` answers the question a user of a browser app has: what would I lose if the browser deleted this copy now? It reports whether the browser granted persistent storage, how many of this member's changes the relay has not confirmed, and whether they are at risk. A page that asks for persistent storage only when the user acts opens with `persistentStorage: "on-request"`:

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({
  model,
  lock,
  name: "tasks",
  persistentStorage: "on-request",
});

const warning = document.createElement("p");
const keep = document.createElement("button");
keep.textContent = "Keep my data on this device";
document.body.append(warning, keep);

db.durability.subscribe((durability) => {
  warning.textContent = durability.atRisk
    ? `${durability.unconfirmed} changes exist only in this browser, which may delete them.`
    : "";
  keep.hidden = durability.storage === "persistent";
});
keep.addEventListener("click", () => {
  db.durability.requestPersistent().catch(() => {});
});

// A view rebuild after a model change runs in the background.
const rebuild = db.view.current().rebuild;
if (rebuild !== null) console.log(`rebuilding: ${rebuild.folded} of ${rebuild.total}`);
```

`storage` is `"persistent"`, `"best-effort"` (the browser may delete the copy) or `"temporary"` (in memory, in a private window). `atRisk` is true while changes are unconfirmed and storage is not persistent. [Browser apps and durability](browser-durability.md) says what each browser does, and what to tell your users. `db.view` reports a rebuild of the view; in the browser it reports `rebuild: null` for now, because the browser runs no rebuild yet.

## 13. Follow your spaces, and leave one

`db.spaces.subscribe(listener)` gives the space list at once, and again when a space is added, left or removed, or this member's role in one changes, as when another member shares a space with it. `space.leave()` removes this member from a space:

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({ model, lock, name: "tasks" });

const list = document.createElement("ul");
document.body.append(list);
const stop = db.spaces.subscribe((spaces) => {
  list.replaceChildren(
    ...spaces.map((space) => {
      const item = document.createElement("li");
      item.textContent = `${space.id} (${space.role ?? "left"})`;
      return item;
    }),
  );
});
addEventListener("pagehide", stop);

const leave = new URLSearchParams(location.search).get("leave");
if (leave !== null) {
  const project = await db.spaces.get(leave);
  const left = await project.leave();
  console.log(left.notice.text);
}
```

The notice says what leaving does not do: this member keeps its copy of everything it received, and until an admin rotates the space key, the others write under a key it still holds. The last admin of a space cannot leave it; the call fails with `lastAdmin`.

## 14. Read the erasure audit

When a person's personal fields are erased, every member destroys its copy of the person's key and says so. `space.erasures.list()` is the audit: for each erasure, whether it is final, provisional or taken back, who performed and who signed it off, and which members confirmed that they deleted the key:

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({ model, lock, name: "tasks" });

const space = new URLSearchParams(location.search).get("space") ?? "";
const project = await db.spaces.get(space);
for (const erasure of await project.erasures.list()) {
  const missing = erasure.members.filter((m) => m.acknowledged === "missing");
  console.log(
    `${erasure.shred}: ${erasure.state}, ${missing.length} members have not confirmed`,
  );
}
```

A member that never comes back online is listed as `"missing"` for good: that is what the audit is for. Every member may read it, a reader too.

## 15. Pair a second device of your own account

A person's devices share one account. The account key stays on the person's devices. A new device opens its database for pairing and shows its invitation, as a QR code or text. It may sync while it waits:

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({
  model,
  lock,
  name: "tablet",
  pairing: "request",
  relay: "ws://localhost:7700",
});
console.log(String(await db.invitation()));

// The answer the laptop shows, once it paired this device.
const answer = new URLSearchParams(location.search).get("answer");
if (answer !== null) {
  const { notices } = await db.account.completePairing(
    nc.pairingAnswer(answer),
  );
  for (const notice of notices) console.log(notice.text);
}
```

A device that holds the account key pairs it, after a person scanned its code or confirmed on both screens that it is their own device. The answer carries the account key, sealed so that only the new device opens it:

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

// The tablet's invitation, as its page printed it.
const text = new URLSearchParams(location.search).get("invitation") ?? "";
if (confirm("Pair this device with your account?")) {
  const paired = await db.account.pair({
    kind: "device",
    invitation: nc.invitation(text),
  });
  console.log(String(paired.answer));
  console.log(`added to ${paired.added.length} spaces`);
}
```

The laptop adds the new device to every space where it is an admin, each with its notice in `paired.added`. `paired.askAdmins` lists the spaces whose admins your app asks to add it.

## 16. Remove a lost device

The first open of an account creates its recovery code: twelve words, which `db.account.recoveryCode()` gives your app until the person confirms they stored them with `confirm()`. When a device is lost, another device of the account removes it with those words. That removes it from every space this device administers and gives the account a new account key, so the lost device's is no longer accepted:

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

// The lost device's member id, from space.members.list().
const lost = new URLSearchParams(location.search).get("member") ?? "";
const words = prompt("Your recovery code") ?? "";
const { removed, askAdmins, notice } = await db.account.removeMember(lost, {
  recover: words,
});
console.log(`removed from ${removed.length} spaces`);
console.log(`ask the admins of ${askAdmins.length} more spaces`);
console.log(notice.text);
```

Each space in `removed` comes with the notice `memberRemoved`. `askAdmins` lists the spaces where no device of the account is an admin, whose admins your app asks to remove it. The notice `accountKeyRenewed` says that members that have not learnt the new account key yet still accept the old one. The words are never logged; a code that is not this account's fails with `notAllowed`, and nothing is removed.

## What the browser SDK does not do yet

RFC 0009 names more than the browser SDK has today. This guide leaves out what it lacks:

- **Node.js.** `nc.open` runs in a browser. The options `path`, `keyFile`, `inMemory` and `agent` of RFC 0009 are not there yet.
- **`space.inspect`,** the details of a conflict.
- **Erasing a person.** The browser SDK reads the erasure audit (step 14) but cannot start an erasure or take one back yet.

## Where to go next

- [apps/tasks-ts](../apps/tasks-ts/README.md) is a finished page with the model above, with a test that drives it in Chromium.
- [RFC 0009](rfc/0009-sdk-api.md) describes every call, error and report.
- [Browser apps and durability](browser-durability.md) says what a browser copy keeps, what is lost when, and what to tell users.
- [The FAQ](faq.md) answers what the relay sees, how conflicts merge and how the schema changes.
