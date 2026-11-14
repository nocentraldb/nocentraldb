# RFC 0009: The SDK API for apps

REQUEST FOR COMMENTS · NOVEMBER 2026 · Ticket NOC-64

This RFC defines the API an app or an agent uses to work with NoCentralDB: how it opens a database, writes and reads objects, follows them live, creates spaces and adds members, and learns how far its copy is synced and how safe it is. It defines the API in TypeScript (`@nocentraldb/sdk`) and in Rust (`nocentraldb`), the two SDKs [GENESIS.md](../../factory/GENESIS.md) names.

The decided RFCs define what lies under these calls, and this RFC names the calls in between. [RFC 0005](0005-schema-and-model-lock.md) names the schema functions and `model.lock.json`. [RFC 0006](0006-local-view.md) names `db.durability` and the options of `nc.open` ([RFC 0006, "Durability of a browser copy"](0006-local-view.md#durability-of-a-browser-copy)). [RFC 0010](0010-accounts.md) designs accounts, and names the calls of pairing and recovery. [RFC 0001](0001-nocentraldb.md#the-design-in-one-page) shows `db.find` with `take` and `after`. [RFC 0002](0002-change-and-log-format.md) defines the change a write becomes, [RFC 0004](0004-merge-rules-basic-types.md) how each field merges and what a read can show, [RFC 0003](0003-keys-and-cryptography.md) and [RFC 0008](0008-membership.md) the keys, roles and members, and [RFC 0007](0007-relay.md) the relay. Every name below links the RFC it builds on, and where an RFC is silent, this RFC says so. GENESIS.md counts public API names among what a user sees first, and RFC 0004, RFC 0005 and RFC 0008 left the names of reads and roles to the SDK, so this RFC is where they are decided.

**Status.** Decided by the owner (Carsten, 2026-11-10).

**How to give feedback.** Reply in this RFC's discussion, [RFC 0009: The SDK API for apps](https://github.com/nocentraldb/nocentraldb/discussions/149), one topic per reply, or open a pull request against this file to change its text. [Giving feedback](README.md#giving-feedback) says how, and what happens to your point.

## What this RFC does not cover

- **Computed fields and views.** RFC 0005 reserves `computed`, `views`, `nc.reach` and `nc.view`; a later RFC designs them. This RFC lets a view be queried like an index, with `take` and `after`, as RFC 0001 shows, and designs nothing else about it.
- **Rich text and streams.** `nc.richText` and `nc.stream` are reserved (RFC 0005). Their write and read calls, such as an anchored edit or appending a chunk, belong to their RFCs.
- **Work orders and erasure.** How an erasure is started, claimed and signed off is the work-order RFC's. RFC 0003's sketch `db.workOrders.erase` stays a sketch.
- **Accounts.** How a second device or an agent joins a person's account, and recovery, are [RFC 0010](0010-accounts.md)'s. This RFC lists the calls it names ([Accounts and devices](#accounts-and-devices)).
- **Peers in island mode.** How two members find each other is a later RFC ([RFC 0007, "Island mode"](0007-relay.md#island-mode)), so the SDK has no peer API yet.
- **Bindings for UI frameworks**, such as a React hook. They can wrap `subscribe` later.

## Terms

| Term | Meaning in this RFC |
| --- | --- |
| Database, `db` | What `nc.open` returns: one member's copy of all its spaces on one device ([RFC 0006, "Where the view lives"](0006-local-view.md#where-the-view-lives)). |
| Space handle | What `db.spaces.get` and `db.spaces.create` return: the calls of one space. The examples name it `project`. |
| Transaction | The writes an app collects in one `space.write` call. It becomes exactly one change ([RFC 0002, "The atomic unit"](0002-change-and-log-format.md#the-atomic-unit)). |
| Query | A declared index, the values of its equality prefix, a page size and an optional cursor ([RFC 0006, "Declared indexes"](0006-local-view.md#declared-indexes)). |
| Page | At most N objects of a query in index order, and the cursor after the last one ([RFC 0006, "Ordered pages"](0006-local-view.md#ordered-pages)). |
| Report | A value the SDK keeps current and an app can read or follow, with `current()` and `subscribe(listener)`, in the shape [RFC 0006, "What the SDK reports"](0006-local-view.md#what-the-sdk-reports) gives `db.durability`. |

## The API in one page

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "./model";
import lock from "./model.lock.json";

const db = await nc.open({ model, lock, relay: "wss://relay.example.com" });
const project = await db.spaces.create();

const { ids } = await project.write((tx) => {
  const task = tx.create(model.tasks, { title: "Write RFC 0009", status: "open", priority: 1 });
  tx.update(model.tasks, task, { tags: nc.add("docs") });
});

const page = await db.find(model.tasks.indexes.byStatusPriority.equal("open").take(50));
const stop = db.subscribe(model.tasks.indexes.byStatusPriority.equal("open").take(50), (fresh) => render(fresh));

await project.members.add(invitation, { role: "writer" });
db.sync.current(); // { connection: "connected", unconfirmed: 0, ... }
```

- **One database per member.** `nc.open` opens this member's copy of all its spaces from the app's `model.lock.json`, and checks the lock on the device, as RFC 0005 requires.
- **Objects live in a space.** Reads and writes of single objects go through a space handle. Queries go through `db`, because an index spans every space of the member, unless the index is declared per space. An object id is unique only in its space, every write is one change in one space, and every right is per space, so the space is the natural receiver; queries span spaces because RFC 0006 keeps one database per member for exactly that. Everything on `db`, with an object reference `{ space, id }`, was not taken.
- **One write, one change.** `space.write` collects creates, updates and deletes and turns them into one signed, encrypted change. It resolves when the change is in the local log and folded into the view; the relay comes later.
- **Plain values, details on demand.** A read returns a plain object, typed by `nc.Infer`. Conflicts, lost removals and link statuses are flagged on it and read in full with `space.inspect`.
- **Reports, not exceptions, for what syncing finds.** Parked, rejected and refused changes, gaps, exposure and the relay connection are reports an app reads or follows. Errors are thrown only for what the app asked and the SDK refuses.
- **The same API in Rust**, in snake case, as [The Rust SDK](#the-rust-sdk) shows.

## Open and close

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "./model";
import lock from "./model.lock.json";

const db = await nc.open({
  model,                              // the TypeScript model: types and handles
  lock,                               // model.lock.json: the rules the core folds by
  relay: "wss://relay.example.com",   // optional; without it, the database stays local
  name: "default",                    // in a browser, which database; "default" if left out
  persistentStorage: "on-open",       // RFC 0006; or "on-request"
  warnings: true,                     // RFC 0006
});

await db.close();
```

**What `nc.open` does,** in this order:

1. **Opens the member's database** on this device, and creates the member key on first open ([RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys)). In a browser it starts the worker, and if another tab owns the database, it reaches that tab instead ([RFC 0006, "The browser"](0006-local-view.md#the-browser)).
2. **Checks the model against the lock.** The core folds only by the lock ([RFC 0005](0005-schema-and-model-lock.md), "The core and both SDKs read only the lock"). The TypeScript model gives the app its types and its handles, such as `model.tasks`. `nc.open` checks that every collection, field and index the model names is in the lock with the same type and options, and fails with `lockOutOfDate` otherwise, because a developer who changed the schema and did not run `ncdb model build` would write fields no lock knows. The lock gives the rules and the model the types and handles, which keeps both what RFC 0005 says (the core reads only the lock) and what RFC 0006 names (`nc.open({ model, ... })`). Two other forms were not taken: `nc.open({ lock })`, typed with a generic, with handles from the lock by name, which loses the handles that make queries typed; and a module that `ncdb model build` also writes and that exports both, which adds a generated file to every app's repository. An SDK that reads the TypeScript model alone and builds the lock in memory was not taken either: the lock is the contract every runtime reads, and building it on the device would skip the build's history and its check ([RFC 0005, "Two checks: at build and on the device"](0005-schema-and-model-lock.md#two-checks-at-build-and-on-the-device)).
3. **Checks the lock on the device.** It runs the additive check between the newest lock the database has been opened with and this one, and refuses with `modelRefused`, naming each refused change, if the `model.id` differs, or the lock is neither an additive successor of the newest lock nor an older lock the newest one descends from. A newer lock takes the newest lock's place; an older one opens the database as an old client and leaves the newest lock as it is. It deletes and rewrites nothing ([RFC 0005, "Two checks: at build and on the device"](0005-schema-and-model-lock.md#two-checks-at-build-and-on-the-device)).
4. **Brings the view up to date.** It adds columns, tables and indexes for an additive change, folds parked operations that are now known, and starts a full rebuild where RFC 0006 asks for one ([RFC 0006, "Rebuilding the view"](0006-local-view.md#rebuilding-the-view)). `nc.open` resolves before a rebuild ends; the app reads the old view meanwhile, and `db.view` reports the rebuild's progress.
5. **Starts syncing** in the background when a relay is given ([Sync and status](#sync-and-status)), and asks the browser for persistent storage unless the app opened with `persistentStorage: "on-request"` ([RFC 0006, "What the SDK reports"](0006-local-view.md#what-the-sdk-reports)).

**Where the database lives.** In Node.js, the TypeScript SDK runs the core as a native addon, built with napi-rs, that wraps the Rust SDK: SQLite on disk, the platform keystore and native speed, the same code as the Rust SDK. On a platform without a prebuilt binary, it falls back to the wasm core, as the browser runs it. So `nc.open` takes the options of the Rust SDK under the names `path`, `keyFile`, `inMemory` and `agent`, with the same meaning: `path` names a directory the SDK owns, `keyFile` keeps the keys in that file for a headless agent with no keystore, `inMemory` keeps the database and its keys in memory for tests, and `agent` certifies the member as an agent ([The Rust SDK](#the-rust-sdk)). Both SDKs name the same options the same way.

- **Opened with neither `path` nor `inMemory`,** it is refused with `invalidValue`: the app must say where its data lives, so no data lands where the app did not say. In memory by default would be safe for tests, but an app that forgets its path loses its data silently; the platform's app data directory by default would be convenient, but is not where the SDK keeps a database ([The Rust SDK](#the-rust-sdk)).
- **With no platform keystore reachable and no `keyFile`,** it fails with `keyStoreUnavailable`, whose message names the `keyFile` option. A headless machine often has no keystore, and the code tells the developer what to do, while a key reaches disk only in a file the app chose. A key file in the database's directory, made without being asked, was not taken, because a key on disk that nobody chose is the risk RFC 0003 avoids ([RFC 0003, "The member key"](0003-keys-and-cryptography.md#the-member-key)); nor was `storageFailed`, which needs no new code but does not tell the developer what to do.
- **Node.js only.** The browser has no path, and keeps its key as [RFC 0006, "What the view stores in plaintext"](0006-local-view.md#what-the-view-stores-in-plaintext) says.

**In a browser,** `name` names the database: 1 to 64 characters of `a`–`z`, `0`–`9`, `-` and `_`, `"default"` when the app leaves it out, and any other name fails with `invalidValue` ([RFC 0006, "The browser"](0006-local-view.md#the-browser)). The SDK runs the core in a dedicated worker it starts itself. `@nocentraldb/sdk` ships the worker script and the core's WebAssembly module in its build output, and finds them with `new URL(..., import.meta.url)`, which bundlers such as Vite resolve, so the app passes no URL (Carsten, 2026-11-12, NOC-274). An option that names the worker's URL was not taken: every app would have to copy files the package already carries.

**Spaces of another model.** A space records the `model.id` it was created with in its genesis ([RFC 0008, "What it names"](0008-membership.md#what-it-names)). `nc.open` does not fail because one space belongs to another model. That space's changes are not folded, nothing is written into it, and `db.spaces.get` on it throws `spaceOfAnotherModel`, an error that names both model ids, as NOC-57 builds it. `db.spaces.list()` shows the space with that state, so an app can tell its user which spaces it cannot open.

**No persistent storage** is not an error. A browser copy opens as `best-effort` or, where OPFS is missing, as `temporary` in memory, and `db.durability` reports it ([RFC 0006, "What the SDK reports"](0006-local-view.md#what-the-sdk-reports)). A full disk or quota fails a write with `storageFull`, never silently ([RFC 0006, "The browser"](0006-local-view.md#the-browser)).

**The relay URL in a browser** is `wss://`, except for `ws://localhost` while developing: a page served over `https://` cannot open a `ws://` connection, and the browser refuses it. The SDK adds no rule of its own; a `ws://` URL the browser refuses leaves the database offline.

**`db.close()`** ends the relay connection, the subscriptions and, in a browser, the tab's ownership of the database, so another tab can take it over. Every write that resolved is already committed, so closing loses nothing; unconfirmed changes stay in the log and are pushed on the next open ([RFC 0007, "Island mode"](0007-relay.md#island-mode)). Every call on a closed database fails with `closed`.

## Write

### One write, one change

```ts
const result = await project.write((tx) => {
  const task = tx.create(model.tasks, { title: "Ship the SDK", status: "open", priority: 2 });
  tx.update(model.tasks, otherTaskId, { dependsOn: nc.add(task) });
  tx.delete(model.tasks, staleTaskId);
});

result.change; // the change id
result.ids;    // the ids of the objects this write created, in order
```

`space.write` takes a function that records writes on `tx`. When the function returns, the SDK turns the records into **one change** of that space: the core signs and encrypts it, appends it to the log and folds it in one transaction, and `write` resolves ([RFC 0006, "The member's own writes"](0006-local-view.md#the-members-own-writes)). The app reads its write from the view at once, with or without a relay. A write is never sent before it is folded.

The calls on `tx`, and the operations they become ([RFC 0004, "Operation numbers"](0004-merge-rules-basic-types.md#operation-numbers)):

| Call | Operations in the change |
| --- | --- |
| `tx.create(collection, values)` | Create (0), then one operation per field in `values`. Returns the new object's id, 128 random bits drawn on the device ([RFC 0002, "Object ids"](0002-change-and-log-format.md#object-ids)). |
| `tx.update(collection, id, values)` | One operation per field in `values`. |
| `tx.delete(collection, id)` | Delete (4) with `true`. |
| `tx.restore(collection, id)` | Delete (4) with `false` ([RFC 0004, "Creating and deleting an object"](0004-merge-rules-basic-types.md#creating-and-deleting-an-object)). |

`project.create(collection, values)`, `project.update(collection, id, values)`, `project.delete(collection, id)` and `project.restore(collection, id)` are shorthands for a `write` with one call; each is one change.

One call maps to one change, so the app sees exactly what is atomic. The forms not taken:

| Rejected | Why |
| --- | --- |
| An async callback that may read inside the transaction | It suggests an isolation that does not exist, because a fold may run between the reads. |
| Only single calls, each one change | "Create a task and link it" would be two changes that another member may receive apart. |
| An ORM in which an object is a class instance that saves itself | Hides when a change is written and how many changes a save makes; one write must be one visible change, with its own checks. |
| A write that resolves only when the relay accepts it | Puts the network on the write path, so island mode would stall every write; RFC 0006 folds the member's own write first. A promise per write is offered instead (`db.sync.confirmed`). |
| A compare-and-set write ("only if the value is still X") | No member can know what an offline member wrote; it would fail or succeed differently per member ([RFC 0001, "What it is not"](0001-nocentraldb.md#what-it-is-not)). |
| Transactions across spaces | A change never spans two spaces ([RFC 0002, "Operations: object, field and value"](0002-change-and-log-format.md#operations-object-field-and-value)). |
| One space handle per object type, such as `db.tasks.create` | Mixes the model into the database's namespace, where a collection named `spaces` or `sync` would collide. |

**What a transaction is, and is not.**

- **Atomic in its checks, in one space.** The signature, the membership check and the decryption cover the change as a whole, so every member accepts all of it or none ([RFC 0002, "The atomic unit"](0002-change-and-log-format.md#the-atomic-unit)). A change never spans two spaces, so neither does a write. An app that writes into two spaces makes two writes, and each may be accepted or rejected on its own.
- **Not isolated.** The function records; it does not read. It gets no view of the transaction's own writes, and it must not await anything: the SDK collects the records when the function returns. Read first, then write.
- **No guard against concurrent writes.** Another member may write the same field at the same time, offline. The fields merge by their types, and a write that loses stays identifiable ([RFC 0002, "A late change stays identifiable"](0002-change-and-log-format.md#a-late-change-stays-identifiable)). There is no "write only if unchanged", because no member can know what an offline member does (RFC 0001, [What it is not](0001-nocentraldb.md#what-it-is-not)).
- **An old client may show part of it.** A transaction that writes a field an old client does not know shows on that client without that field, until it upgrades ([RFC 0002, "The atomic unit"](0002-change-and-log-format.md#the-atomic-unit)).
- **The net effect per field.** A transaction that writes one field twice writes its last value; an element added and then removed in one transaction is written as RFC 0004 says ([RFC 0004, "The arguments of each operation"](0004-merge-rules-basic-types.md#the-arguments-of-each-operation)). An operation cannot name another operation of its own change, so the SDK encodes the net effect.

### Field operations per field type

`values` maps field names to what to write. The field's type decides what a value means ([RFC 0005, "Field types"](0005-schema-and-model-lock.md#field-types); [RFC 0004](0004-merge-rules-basic-types.md)):

| Field | Write | Operation |
| --- | --- | --- |
| `nc.string`, `nc.int`, `nc.float`, `nc.boolean`, `nc.oneOf` | The value, such as `{ priority: 3 }`. `null` clears the field. | set (1). `replaces` names the winner and every conflict the copy holds ([RFC 0004, "Basic values"](0004-merge-rules-basic-types.md#basic-values)). |
| `nc.link` | The target's object id, or `null`. | set (1), as a basic value. |
| `nc.ref` | The `{ space, id }` a read gives, the target's space id and object id, or `null` to clear it: a write takes what a read gives. | set (1), as a basic value ([RFC 0004, "References"](0004-merge-rules-basic-types.md#references)). |
| `nc.set`, `nc.links` | `nc.add(...elements)` or `nc.remove(...elements)`. | add (2) per element, or remove (3) per element with `observed`, the tags of its additions the copy holds ([RFC 0004, "Sets"](0004-merge-rules-basic-types.md#sets)). |
| A plugin's type | A `Data` value, which the plugin's write function turns into operations ([RFC 0011, "The field type a plugin implements"](0011-plugin-field-types.md#the-field-type-a-plugin-implements)). | The plugin's operations, 64 and up. |

- **Every write resolves the conflicts the copy holds.** A set replaces the winner and every conflict on this device, as RFC 0004 defines it. An app that shows conflicts to a person before writing gets what RFC 0004 intends: the person decides with the conflict in front of them. An app that writes without looking resolves them too; [Read and query](#read-and-query) says how to find them first.
- **A deliberate re-add.** `nc.add(e)` names the lost removals of `e` the copy holds in `replaces`, so they stop counting as lost ([RFC 0004, "Sets"](0004-merge-rules-basic-types.md#sets)).
- **A set is never written whole.** There is no "set the tags to exactly these", and no whole array that the SDK turns into additions and removals: it would remove what a concurrent member added without its author having seen it, so the intent "exactly these" cannot converge under RFC 0004's add wins.
- **A reference is written as it reads.** A read object of the target, from which the SDK would take `$space` and `$id`, was not taken: shorter for an app that holds the target, but a write would depend on a read of another space.
- **`undefined` writes nothing.** A key with the value `undefined` is ignored, so an app can pass a partial object. Every field may be unset ([RFC 0005, "No required fields and no defaults"](0005-schema-and-model-lock.md#no-required-fields-and-no-defaults)).
- **Strings are NFC-normalised** before they are written ([RFC 0005, "Field types"](0005-schema-and-model-lock.md#field-types)).

**What the SDK refuses,** before anything is written, with an error and no change:

- a value that does not fit the field's kind, such as a text in an `nc.int` field, an option the lock does not list, or a value nested deeper than RFC 0004 allows: `invalidValue` ([RFC 0005, "Data from a newer model"](0005-schema-and-model-lock.md#data-from-a-newer-model), point 5). In TypeScript an `nc.int` is a `number`, so a value for it, or for an element of a set of ints, must be a safe integer, between −(2^53 − 1) and 2^53 − 1; any other number is refused with `invalidValue` ([RFC 0005, "Types for the app"](0005-schema-and-model-lock.md#types-for-the-app)).
- a deprecated field or collection: `deprecated` ([RFC 0005, "Modifiers"](0005-schema-and-model-lock.md#modifiers));
- an update, delete or restore of an object the copy does not hold: `notFound`. The format allows such a write ([RFC 0004, "Creating and deleting an object"](0004-merge-rules-basic-types.md#creating-and-deleting-an-object)), but an app that writes to an id it cannot read has most likely a wrong id. This is a rule of the SDK, not of the format;
- a write into a space where this member may not write sealed changes, because it is a reader or no longer a member: `notAllowed` or `notMember` ([RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights));
- a write to a plugin field whose type and rule this member's core has not registered: `unknownType` (Carsten, 2026-11-12, NOC-232; [RFC 0011, "A type that is not registered"](0011-plugin-field-types.md#a-type-that-is-not-registered));
- a write into a space of another model: `spaceOfAnotherModel` ([RFC 0005, "Data from a newer model"](0005-schema-and-model-lock.md#data-from-a-newer-model)).

### Personal fields

A personal field is written like any other field. The SDK seals it under the key of the person it is about, which it finds from the schema's `person` ([RFC 0005, "Personal fields"](0005-schema-and-model-lock.md#personal-fields)):

- `person: "self"`: the object itself is the person.
- `person: "<link field>"`: the target of that link, as the copy holds it after this transaction's own writes. A transaction that sets `assignee` and `notesAboutAssignee` together seals the notes under the new assignee's key.

The SDK holds the person keys of a space in memory only, fetched from the relay's person-key store on every connection, and writes none to the device ([RFC 0003, "The person key"](0003-keys-and-cryptography.md#the-person-key)).

A personal field whose `person` link is unset is refused with `noPerson` ([RFC 0005, "What the build checks"](0005-schema-and-model-lock.md#what-the-build-checks)): sealing it under no person would make it unerasable, and choosing a person would guess. A write of a personal field whose person has a person key this device does not hold yet is refused with `personKeyNotHeld`; the app can retry once sync has fetched the key (Carsten, 2026-11-12, NOC-255).

**How a personal field reads** (Carsten, 2026-11-12, NOC-244):

- **Erased.** A field whose person's key a shred erased, provisionally or for good, reads as unset, as if it held no value, and the read object names it in `$erased`, the names of its erased fields, as `$raw` names fields outside their kind. The view keeps no plaintext of it, so no index holds it ([RFC 0006, "What the view stores in plaintext"](0006-local-view.md#what-the-view-stores-in-plaintext)). A provisional erasure keeps the key, but the field still reads as erased until a take-back ([RFC 0008, "A provisional erasure in a space with one account"](0008-membership.md#a-provisional-erasure-in-a-space-with-one-account)). An app that does nothing shows an empty field; an app that cares can say "erased". A distinct error on reading such a field was not taken: every read of a person's object would need error handling.
- **Its key not held yet.** A sealed value whose person key this device has not fetched yet reads as unset, not as erased, and the object reads again once the key arrives. A status that lists such fields was not taken: it describes a state that lasts only until the next fetch.
- **Two first keys.** When two devices create a person's key at once, the first person-key entry in the order chooses the key ([RFC 0003, "Shredding a person's key"](0003-keys-and-cryptography.md#shredding-a-persons-key)), and values sealed under the other key can never be opened. They read as unset, a limit stated plainly. Re-sealing them under the chosen key, by the device that wrote them, was not taken for v1: a writer that changes values on its own is a design point of its own.

The erasure itself is not a call of this RFC: it belongs to the work-order RFC ([What this RFC does not cover](#what-this-rfc-does-not-cover)).

**The same person in another space.** RFC 0005 gives a person a person id that the core carries over when the app records the same person in another space ([RFC 0005, "Identifying the person"](0005-schema-and-model-lock.md#identifying-the-person)), and this RFC says how the app says "the same person". `tx.create` takes the option `samePersonAs`, an object of the same person read from another space:

```ts
await support.write((tx) => {
  tx.create(model.customers, { name: "Anna Schmidt" }, { samePersonAs: annaInSales });
});
```

The SDK reads the person id from `annaInSales` and keeps it on the new object ([RFC 0005, "Identifying the person"](0005-schema-and-model-lock.md#identifying-the-person)). Without the option, a new person id is drawn. An erasure that names the person then reaches both spaces ([RFC 0003, "The person key"](0003-keys-and-cryptography.md#the-person-key)).

## Read and query

Every read is a query against the view on this device. It never waits for the network and never asks the relay for anything, also when the copy is empty or behind ([RFC 0006, "Reads never touch the network"](0006-local-view.md#reads-never-touch-the-network)).

### Get by id

```ts
const task = await project.get(model.tasks, taskId);
// { $id, $space, title: "Ship the SDK", status: "open", priority: 2, tags: ["docs"], dependsOn: [...] }
```

`space.get(collection, id)` returns the object as `nc.Infer` types it, or `undefined` when the copy holds no object with that id in that collection ([RFC 0005, "Types for the app"](0005-schema-and-model-lock.md#types-for-the-app)):

- every field optional; an unset field is `undefined`;
- a choice as the union of its options, and `nc.Unknown`, `{ unknown: rawValue }`, for an option the lock does not list yet: unknown options are common after an additive change, so the type makes the app handle them;
- a set as an array, in the order of its elements' canonical bytes; a link as the target's object id; many links as an array of ids;
- a reference as `{ space, id }`, the target's space id and object id, in the form `$space` and `$id` carry them. Its state is not part of the read object, so the object reads the same on every member of its space, and the SDK does not read the target: an app that wants it reads it with `space.get` in the target's space ([RFC 0005, "References into another space"](0005-schema-and-model-lock.md#references-into-another-space)).

In TypeScript an object id is a `string`: its 16 bytes as 32 lowercase hex digits, as the lock writes `model.id` ([RFC 0005, "The keys"](0005-schema-and-model-lock.md#the-keys)). An id can then be a map key, compared with `===`, and logged as it is, and one encoding serves both the lock and the ids. A base64url string, a `Uint8Array` and a branded string type were not taken. The Rust SDK keeps the 16 bytes as they are.

A space id has the same form in TypeScript: a `string` of 32 lowercase hex digits, its 16 bytes in order. `$space`, `space.id` and a reference's `space` read this way, and a write takes a space id this way: one rule for both ids, and a reference reads `{ space, id }` in one form.

An object id is unique in its space, not across spaces: a hostile member can create an id that exists in another space ([RFC 0002, "Object ids"](0002-change-and-log-format.md#object-ids)). So a single object is read through its space, and an object read from a query carries its space.

**What a read object carries beyond its fields.** Names that start with `$` are reserved for the core and never name a field ([RFC 0005, "Names"](0005-schema-and-model-lock.md#names)), so the SDK puts what it knows about the object there:

| Property | Meaning |
| --- | --- |
| `$id`, `$space` | The object id and the space id. |
| `$deleted` | `true` when the object is deleted. `space.get` returns deleted objects; queries do not ([Queries and indexes](#queries-and-indexes)). |
| `$conflicts` | The names of the fields that have conflicts, lost removals, or a link that is not **ok**. Empty when there are none. A reference is named for its conflicts only, never for its state, because a state is not a conflict and differs from member to member ([RFC 0005, "References into another space"](0005-schema-and-model-lock.md#references-into-another-space)). |
| `$raw` | The names of the fields whose value does not fit the field's kind, such as a text in an `nc.int` field written by a buggy member. Such a field reads `undefined`; `space.inspect` gives the raw value ([RFC 0005, "Data from a newer model"](0005-schema-and-model-lock.md#data-from-a-newer-model), point 5). In TypeScript this includes an `nc.int` beyond −(2^53 − 1) to 2^53 − 1, which another member or the Rust SDK can write, and a set of ints holding such an element, as a whole; `space.inspect` gives such an int exactly, as a `bigint` ([RFC 0005, "Types for the app"](0005-schema-and-model-lock.md#types-for-the-app)). |

Values outside the kind come only from buggy or hostile members, and `$raw` still makes them visible (principle 3). Two other shapes were not taken. Every field wrapped as `{ value, conflicts }` in every read: most reads show values and conflicts are rare, so the `$conflicts` flag tells the app where to look and `inspect` gives the detail. A value outside the kind returned as it is, typed `unknown`: it burdens every read for a rare case.

### Conflicts, lost removals and link states

`space.inspect(collection, id)` returns everything RFC 0004 lets an app read about one object, per field ([RFC 0004, "Late changes stay identifiable"](0004-merge-rules-basic-types.md#late-changes-stay-identifiable)):

```ts
const detail = await project.inspect(model.tasks, taskId);
detail.fields.title;     // { value: "Ship the SDK", conflicts: [{ value: "Ship it", author, stamp }] }
detail.fields.tags;      // { elements: ["docs"], lostRemovals: { docs: [{ author, stamp }] } }
detail.fields.assignee;  // { value: personId, status: "deleted", conflicts: [] }
detail.collection;       // { value: "tasks", conflicts: [] }  (RFC 0004, two creates of one id)
detail.deleted;          // { value: false, conflicts: [] }
```

- **Basic values and single links:** `value` and `conflicts`, each conflict with its value, author member id and stamp. Conflicts whose value equals the winner's are left out, as RFC 0004 says.
- **Sets and many links:** `elements`, and per element its `lostRemovals`, each with author and stamp.
- **Links:** per target its `status`: `"ok"`, `"deleted"`, `"missing"` or `"wrongCollection"` ([RFC 0004, "Resolution"](0004-merge-rules-basic-types.md#resolution)).
- **References:** `value`, `conflicts`, and the state of the winning reference as `status`: `"ok"`, `"notAccessible"` or `"missing"` ([RFC 0004, "References"](0004-merge-rules-basic-types.md#references); [RFC 0005, "References into another space"](0005-schema-and-model-lock.md#references-into-another-space)). The state comes from the member's own copies, so it can differ from member to member and change without any change to the reference.
- **The object:** its collection and whether it is deleted, each a basic value with conflicts ([RFC 0004, "Two creates of one object id"](0004-merge-rules-basic-types.md#two-creates-of-one-object-id)).
- **A field outside the kind:** `{ raw: value }`.

`project.outcome(change, collection, id, field)` answers RFC 0004's `outcome(change, field)`: `"won"`, `"lost"` or `"replaced"` for a basic value, `"applied"`, `"lost"` or `"replaced"` for a set. These, and `value`, `conflicts`, `lostRemovals` and the statuses above, with **wrong collection** as `"wrongCollection"`, are RFC 0004's working names, made the public names here: they are plain words, and the RFCs and the FAQ already use them. Other names, such as `losers` for conflicts and `dangling` for missing, were not taken.

### Queries and indexes

A query names a declared index, fixes its equality prefix and asks for a page ([RFC 0006, "Declared indexes"](0006-local-view.md#declared-indexes)). The model gives every declared index a handle:

```ts
const query = model.tasks.indexes.byStatusPriority.equal("open").take(50);
const page = await db.find(query);       // { items: Task[], cursor, hasMore }
const more = await db.find(query.after(page.cursor));
const firstFive = await db.find(model.tasks.indexes.byPriorityDesc.take(5));
```

A query is written from handles of the model and passed to `db.find`: it is the chain RFC 0001 shows for views, TypeScript checks the equality values against the index, and a view will be queried the same way. An options object, `db.find(model.tasks, { index: "byStatusPriority", equal: ["open"], limit: 50 })`, was not taken.

- `.equal(...values)` fixes the index's leading fields, as many as its `equal` declares, no more and no fewer; TypeScript checks the count and the types ([RFC 0005, "Indexes"](0005-schema-and-model-lock.md#indexes)).
- `.take(n)` sets the page size, 50 by default and at most 1000; a larger `n` fails with `invalidValue`, in both SDKs (Carsten, 2026-11-12, NOC-274).
- `.after(cursor)` starts after a page's cursor. A cursor from another model, or another index, fails with `cursorOfAnotherModel`, and the app starts from the first page ([RFC 0006, "Ordered pages"](0006-local-view.md#ordered-pages)).
- `db.find(query)` returns `items` in index order, the `cursor` after the last item, and `hasMore`. Every item carries `$space`, because an index spans every space of the member ([RFC 0006, "Where the view lives"](0006-local-view.md#where-the-view-lives)).
- Queries skip deleted objects, and the index does not hold them; `space.get` returns a deleted object with `$deleted: true`. An app almost never lists deleted objects, and filtering in the app reads rows it throws away. A deletion can be undone ([RFC 0004, "Creating and deleting an object"](0004-merge-rules-basic-types.md#creating-and-deleting-an-object)), so `get` still finds the object to restore it. Returning deleted objects from queries, flagged, with the app filtering, was not taken.
- A view, once the computed-fields RFC designs it, is queried the same way: `model.tasks.views.ready.take(50)`, as RFC 0001 shows.

There is nothing else: no `where` on a field outside the index, no sort of the app's choosing, no raw SQL. An app that needs another filter declares another index, or a computed field and a view ([GENESIS.md, principle 6](../../factory/GENESIS.md#principles-in-priority-order)). A `where` clause on any field needs a scan, which principle 6 rules out. Raw SQL against the view could scan, and read the merge state and the log ([RFC 0006, "Declared indexes and ordered pages"](0006-local-view.md#declared-indexes-and-ordered-pages)).

**A query in one space.** An index ends with the space id and the object id ([RFC 0006, "Declared indexes"](0006-local-view.md#declared-indexes)), so it cannot fix the space: "the open tasks of Project A" is no query such an index allows, and an app needs it on its first screen. So an index option, `scope: "space"`, puts the space id first, and `space.find(query)` fixes it:

```ts
// in the schema: indexes: { openByPriority: nc.index(["status", "priority"], { equal: 1, scope: "space" }) }
const mine = await project.find(model.tasks.indexes.openByPriority.equal("open").take(50));
```

RFC 0005 states the option and its key in the lock ([RFC 0005, "Indexes"](0005-schema-and-model-lock.md#indexes)), and RFC 0006 the index's columns ([RFC 0006, "Declared indexes"](0006-local-view.md#declared-indexes)). Two options were not taken. No query per space, with the app filtering the cross-space page in memory, reads rows it throws away, which principle 6 rules out. Every index per space, with `db.find` querying each space and merging, loses the page across spaces in one range lookup, which is why RFC 0006 keeps one database per member.

**What a page does not promise,** as RFC 0006 says: a row that changes its indexed values between two pages may appear twice or not at all; there is no snapshot over many pages. A cursor holds plaintext field values, never leaves the device in the SDK's design, and prints as `[redacted]`; an app that puts it in a URL makes a copy the core cannot erase.

### Parked data, rejected changes and gaps

What the copy holds but does not show is reported per space, in `space.status` ([Sync and status](#sync-and-status)): the parked changes and operations with their reasons and the highest model version they need, counted apart for "another model" ([RFC 0005, "Data from a newer model"](0005-schema-and-model-lock.md#data-from-a-newer-model), point 6), the rejected changes with their reasons ([RFC 0002, "Rejected changes"](0002-change-and-log-format.md#rejected-changes)), and the gaps. A read never includes them, and the report lets an app say "Some data needs a newer version of this app" instead of hiding it.

## Subscribe

Subscriptions re-run what an app reads when a fold touches it. They react to what arrived and never cause a network request ([RFC 0006, "Folding"](0006-local-view.md#folding), step 6).

```ts
const stop = db.subscribe(model.tasks.indexes.byStatusPriority.equal("open").take(50), (page) => {
  render(page.items);
});

const stopOne = project.subscribe(model.tasks, taskId, (task) => show(task));

const stopChanges = db.changes.subscribe(({ space, collection, ids, origin }) => {
  // ids only: never values (RFC 0006); origin is "local" or "remote"
});

stop();
```

- **`db.subscribe(query, listener)`** calls the listener with the query's page at once, and again after every fold that touched an object of the query's collection whose page may change. It passes the whole page, never a diff, so a listener cannot miss a step. Folds of one page of changes call it once ([RFC 0006, "Many changes"](0006-local-view.md#many-changes)). `space.subscribe(query, listener)` does the same for a query in one space.
- **`space.subscribe(collection, id, listener)`** follows one object: the listener gets it as `space.get` returns it, or `undefined`.
- **A subscription does not follow a reference's target.** It re-runs when the referencing object changes, not when the target arrives, changes or is deleted, and not when the member joins or leaves the target's space, so a fold in one space never calls the listeners of another. An app that shows the target subscribes to it in the target's space ([RFC 0005, "References into another space"](0005-schema-and-model-lock.md#references-into-another-space)).
- **`db.changes.subscribe(listener)`** is the change event: after each fold, the space, collection and ids it touched, and whether the change was this member's own write. It carries ids, never values, as RFC 0006's notification does, so an adapter or an agent's wake-up logic reads the objects itself.
- Every `subscribe` returns a function that stops it, like `db.durability.subscribe` ([RFC 0006, "What the SDK reports"](0006-local-view.md#what-the-sdk-reports)).
- **Why whole pages.** Re-running the page is what RFC 0006's notification is for, and an app that gets the whole page cannot apply a diff wrongly. Only a stream of changes, with the app re-reading, was not taken: every app would write the same loop.
- **Un-folding is a change too.** When a late control change rejects a change that was folded, the fields it touched are folded again ([RFC 0006, "Control changes and changes that wait for them"](0006-local-view.md#control-changes-and-changes-that-wait-for-them)), and the listeners of those objects are called like after any fold. The app sees a value go back; `space.status` says why.

## Spaces and members

### Spaces

```ts
const project = await db.spaces.create();           // writes the genesis, offline if need be
const same = await db.spaces.get(project.id);
const all = await db.spaces.list();                 // [{ id, role, model: "ours" | "another", ... }]
const stop = db.spaces.subscribe((all) => showSpaces(all));
const left = await project.leave();
left.notice; // { code: "spaceLeft", text: "This member keeps its copy of everything it received before now. Until an admin rotates the space key, the others write under a key it still holds." }
```

- **`db.spaces.create()`** writes the space's genesis and the creator's own key envelope, on the device, offline if need be ([RFC 0008, "The first change of a space"](0008-membership.md#the-first-change-of-a-space)). The creator is the space's first admin. The space id is derived from the creator and a nonce, so the app does not choose it. A relay that does not let this member create spaces answers `not_allowed` when the genesis is pushed ([RFC 0007, "The errors"](0007-relay.md#the-errors)); the space keeps working locally, and `space.status` reports the refusal.
- **A space has no name in the SDK.** The genesis holds no name, because a name is content the relay must not read ([RFC 0008, "What it names"](0008-membership.md#what-it-names)). The app keeps names in its own model, for example a `projects` collection with one object in each project's space: an app already models what a project is, and its name belongs there. A reserved core collection in each space, such as `$space` with `name` and `description`, was not taken: it would not be in the app's lock, so RFC 0002's fold would park it as unknown. Names can come later as an addition.
- **`space.leave()`** writes a removal of the member itself ([RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights)). The SDK refuses with `lastAdmin` when this member is the space's last admin, as RFC 0008 requires; it cannot prevent two admins from leaving at the same time ([RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights)). It returns a notice, as every call that changes who can read does: `spaceLeft`, "This member keeps its copy of everything it received before now. Until an admin rotates the space key, the others write under a key it still holds." (Carsten, 2026-11-11, NOC-219). A member that leaves must not know the next key, so it does not rotate; until an admin rotates, the space stays on a generation the leaver holds, and `space.status` reports it as exposed ([RFC 0008, "Two removals that rotate the key at once"](0008-membership.md#two-removals-that-rotate-the-key-at-once)). The shorter text "This member keeps its copy of everything it received before now." was not taken: it would leave out the limit that lasts until the rotation.
- **`db.spaces.subscribe(callback)`** follows the space list, in the shape of the other reports: it calls the callback with the current list at once, then with each new list when a space is added, left or removed, or its role changes, for example when another device shares a space with this member (Carsten, 2026-11-12, NOC-241). It returns a function that stops it. Not taken: a `spaceAdded` notice, which would cover only one of these changes, and polling `db.spaces.list()`.
- **Deleting a space** that synced is not defined by any RFC yet ([RFC 0008, "What this RFC does not cover"](0008-membership.md#what-this-rfc-does-not-cover)), so the SDK offers no call for it.

### Invitations and members

```ts
// On the joining member, a device or an agent:
const invitation = await db.invitation();   // public: member id, public keys, certificate, fingerprint
send(invitation.toString());                 // the app carries it to an admin

// On an admin of the space:
const added = await project.members.add(nc.invitation(text), { role: "writer" });
added.notice;  // { code: "memberAdded", text: "The new member reads the whole history of this space." }

const lowered = await project.members.setRole(memberId, "reader");
lowered.notice; // { code: "roleSet", text: "A member whose role was lowered keeps its copy of everything it received before now." }
const removal = await project.members.remove(memberId);
removal.notice; // { code: "memberRemoved", text: "The removed member keeps its copy of everything it received before now." }
const members = await project.members.list(); // [{ id, account, linkedTo, kind, role, self }]
```

- **An invitation is the joining member's public identity.** `db.invitation()` returns its member id, its two public keys, its member certificate and its account's fingerprint. Nothing in it is secret, and no key bytes reach the app ([RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys)). The app carries it to an admin by any channel: a link, a QR code, a message. `invitation.toString()` gives its text form, `ncdb-invite-1:` and base64url of a fixed binary layout, and `nc.invitation(text)` reads it back; a text that does not follow the layout fails with `invalidValue` ([RFC 0003, "Invitation"](0003-keys-and-cryptography.md#invitation)). One hop is enough, since nothing in it is secret, and it is what RFC 0003 and use case 3 show, `members.add(invitation, { role })`. Two hops, with the admin creating an invitation that the joining member accepts, would cost two trips through the app's channel; a mailbox at the relay that carries invitations would change RFC 0007 and let the relay see who is about to join which space. Neither was taken.
- **`members.add(invitation, { role })`** checks the certificate, writes the membership entry and the new member's key envelope, and returns a notice ([RFC 0003, "Sharing a space key with a new member"](0003-keys-and-cryptography.md#sharing-a-space-key-with-a-new-member)). `role` may be left out for an agent, which then joins as `"writer"` ([RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights)); for a device it is required. `members.add` also takes a list of invitations, which writes one entry and one envelope per member, so that an app adds several members to a space in one call. That is plain batching.
- **No group object.** The SDK keeps no list of members of its own, because groups are not part of the design ([RFC 0008, "Groups"](0008-membership.md#groups)). A group list stored on the admin device that made it would be lost with that device, and other admins could not see it.
- **An account new to this device** fails with `unknownAccount`, which carries the account's fingerprint, until the app passes `{ role, fingerprint }` after a person compared it, the way messaging apps compare safety numbers ([RFC 0010, "The fingerprint"](0010-accounts.md#the-fingerprint)). Both SDKs compute it the same way: until accounts are built, over the database's own account key; once RFC 0010 is built, over the recovery key. The database remembers each account whose fingerprint a person confirmed, in the table `confirmed_accounts` of its SQLite file ([RFC 0006, "Where the view lives"](0006-local-view.md#where-the-view-lives)), so a later add of a member of that account needs no fingerprint: a person compares once. Remembering nothing, with the app passing the fingerprint every time, was not taken.
- **What `members.add` refuses,** with nothing written: a certificate that does not hold, with `invalidCertificate`, a code of its own because a person may need to see it, as it can mean a forged invitation; and a member that is already a member, with `alreadyMember`. A former member is no error: the SDK rotates the space key first and then adds it, as RFC 0008 requires when a right a member lost is granted again ([RFC 0008, "A member demoted and raised again"](0008-membership.md#a-member-demoted-and-raised-again)).
- **`members.setRole(memberId, role)`** raises or lowers a role; **`members.remove(memberId)`** removes a member and rotates the space key, as RFC 0003 requires. Both write a cutoff ([RFC 0008, "The cutoff"](0008-membership.md#the-cutoff)), and both return a notice: `memberRemoved` for `remove`, and `roleSet` for `setRole`, whose text says that a member whose role was lowered keeps its copy of everything it received before now (Carsten, 2026-11-11, NOC-203). Returning nothing from `setRole` was not taken: lowering a role changes who can write or administer, and every such call returns a notice. Re-granting a right a member lost rotates first ([RFC 0008, "A member demoted and raised again"](0008-membership.md#a-member-demoted-and-raised-again)). Only an admin may call them; others get `notAllowed`.
- **Roles** are `"reader"`, `"writer"` and `"admin"`, RFC 0008's working names, made the public names here: they say what the role may do, and RFC 0008 and the FAQ use them. `"viewer"`, `"editor"` and `"owner"` were not taken; "owner" suggests one person, while a space has many admins.
- **`members.list()`** returns each current member's id, its account fingerprint, the fingerprint of the account that one is linked into when the database knows the link (`linkedTo`, else none; [RFC 0010, "Linking accounts drawn per database"](0010-accounts.md#linking-accounts-drawn-per-database); Carsten, 2026-11-13, NOC-307 A1), its `kind` (`"device"`, `"agent"` or `"subAgent"`, from its certificate), its role, and whether it is this member. Every member is visible, an agent included ([GENESIS.md, "What it must never do"](../../factory/GENESIS.md#what-it-must-never-do)). The id is what `setRole` and `remove` take, so an app names a member by the list it shows, not by an invitation it would have to keep (Carsten, 2026-11-11, NOC-203). It is 64 lowercase hex digits, as an object id is 32: a hash of the member's two public keys, which every invitation carries, so it shows no secret, and an app can store it and send it to another device (Carsten, 2026-11-13, NOC-298). An opaque handle that only the database that made it accepts was not taken: removing a member from another device would not work. A removed member is not listed; listing it with no role, as `spaces.list()` does for a space the member left, was not taken.
- **Notices.** Every call that changes who can read or write returns a notice: `{ code, text }`, the text in plain English for the developer, saying what the change does not do ([RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys)). A notice's code is named by the call that returns it: `memberAdded` for `members.add`, `memberRemoved`, `roleSet`, `spaceLeft`, `adHocSpaceCreated`, `erased`, and so on as calls come. Apps match on the code, and it stays the same when its text is reworded later; a code named by what the text says, such as `readsWholeHistory`, was not taken, because a reworded text could make it read wrong. The SDK ships no text for end users ([RFC 0006, "What developers and their users read"](0006-local-view.md#what-developers-and-their-users-read)).

**What membership calls do not do,** stated as their notices state it: a removed member keeps its copy; a revocation does not take back what a member read; a member that has not seen a removal treats the removed member as it was until the removal reaches it; and until a merge rotation lands, members may write under a key generation a removed member holds ([RFC 0008, "Two removals that rotate the key at once"](0008-membership.md#two-removals-that-rotate-the-key-at-once)).

**Outcomes that change later.** A grant concurrent with a removal of the same member loses, and the SDK shows it as a lost grant ([RFC 0008, "Concurrent add and remove of one member"](0008-membership.md#concurrent-add-and-remove-of-one-member)). `space.status` lists lost grants, and `members.list()` shows the role that resulted, not the one asked for.

### Erasures

```ts
const erasures = await project.erasures.list();
// [{ shred, state: "final" | "provisional" | "takenBack",
//    performer: { id, account }, signOff: account,
//    members: [{ id, account, current, acknowledged: "deleted" | "seen" | "missing" }] }]
```

**`space.erasures.list()`** is the audit of the space's erasures: for each one, which members confirmed that they deleted their copy of the person key, which only saw it, and which never answered ([RFC 0008, "Acknowledgement"](0008-membership.md#acknowledgement); Carsten, 2026-11-12, NOC-246). It is shaped like `members.list()`, a call on the space that returns plain values, and names members the same way, by id and account fingerprint.

- **`shred`** is the change id of the erasure's shred entry. The app matches its own erasure by it; the erase call of the work-order RFC returns the same id.
- **`state`** is `"final"` for a shred, or a provisional shred a decision finalized; `"provisional"` while a provisional erasure waits for its decision; and `"takenBack"` for one its account took back, which destroyed nothing ([RFC 0008, "A provisional erasure in a space with one account"](0008-membership.md#a-provisional-erasure-in-a-space-with-one-account)).
- **`performer`** is the member that performed the erasure, with its account's fingerprint, and **`signOff`** the fingerprint of the account that signed it off ([RFC 0008, "Shred entry"](0008-membership.md#shred-entry)).
- **`members`** lists every member that could have held the key: the members in the state of the shred's causal past, and any member that acknowledged it later. **`current`** says whether it is still a member, and **`acknowledged`** whether it acknowledged `"deleted"`, only `"seen"`, or nothing yet, `"missing"`. A removed device that kept the key and never came back is listed as missing, which is what the audit is for ([RFC 0003, "Shredding a person's key"](0003-keys-and-cryptography.md#shredding-a-persons-key)).
- **Every member may call it**, a reader too: RFC 0008 names auditors among the readers.

Not taken: a field of `space.status`, which the SDK recomputes after every fold while the audit is read now and then; an audit across spaces by person, database-wide, which needs the mapping from a person id to its person-key ids and comes with the work-order RFC; current members only, which hides a removed device that kept the key; every member the space ever had, which lists members removed before the person key existed; and the person's object ids on each erasure, which wait until personal fields map a person to its keys, and can then be added without breaking an app.

### Sub-agents and ad-hoc spaces

```ts
// The parent agent:
const { space: adhoc, notice } = await db.spaces.createAdHoc({ copy: [taskA, taskB], personal: false });
await adhoc.members.add(subAgentInvitation, { role: "writer" });
const result = await adhoc.get(model.tasks, copyOfTaskA);
await project.update(model.tasks, taskA.$id, { status: result.status });
```

- **`db.spaces.createAdHoc({ copy, personal, local })`** creates a space with a fresh key and copies the given objects into it as new objects ([RFC 0003, "Sub-agents get an ad-hoc space"](0003-keys-and-cryptography.md#sub-agents-get-an-ad-hoc-space)). Personal fields are not copied unless `personal: true` ([RFC 0003, "Sub-agents get an ad-hoc space"](0003-keys-and-cryptography.md#sub-agents-get-an-ad-hoc-space)), and then the person key is shared into the space. `local: true` keeps the space off the relay, for a sub-agent on the same machine. The notice says that the space limits what the sub-agent can decrypt, not what a process running as the same user can read.
- **A sub-agent** opens its own database with `nc.open({ model, lock, subAgent: true })`; its invitation carries no certificate, and the parent's `members.add` signs one with the parent's member key ([RFC 0008, "What this RFC uses from RFC 0003"](0008-membership.md#what-this-rfc-uses-from-rfc-0003)).
- **The source of a copy.** RFC 0003 says the copies "keep a reference to their source". The parent's SDK keeps the mapping locally, which is all the parent needs to write results back; the sub-agent never learns the source's id.
- **Deleting the ad-hoc space** is a local deletion while it stayed local (`adhoc.discard()`); a synced one waits for the RFC that deletes spaces.

### Accounts and devices

[RFC 0010](0010-accounts.md) designs accounts: the account key, the recovery code, pairing a device or an agent, and linking the accounts that each database draws today. The rest of the API refers to pairing, because an agent needs a certificate from its account before it can join a space. Its calls:

```ts
// On the new device or agent, a database opened for pairing shows its invitation:
const db = await nc.open({ model, lock, pairing: "request" });
const invitation = await db.invitation();

// On a device of the account, after the person confirmed:
const answer = await db.account.pair({ kind: "device", invitation });

// Back on the new device:
await db.account.completePairing(answer);

// On a new device, recovering the account from its code:
const recovered = await nc.open({ model, lock, recover: words });
```

- **`nc.open({ model, lock, pairing: "request" })`** opens a database for pairing; its invitation carries no certificate yet ([RFC 0010, "Adding a device or an agent to an account"](0010-accounts.md#adding-a-device-or-an-agent-to-an-account)).
- **`db.account.pair({ kind, invitation })`**, on a device of the account, returns the answer: the certificate, and for a device the sealed account key. `kind` is `"device"` or `"agent"`.
- **`db.account.completePairing(answer)`**, on the new device, takes the answer. Linking a database that already has data uses the same calls, so there is no `db.account.link`.
- **`nc.open({ model, lock, recover })`** recovers an account on a new device from its recovery code ([RFC 0010, "The recovery code"](0010-accounts.md#the-recovery-code)).
- **The recovery code** is handed to the app once, when the account is created, as an object whose string, JSON and debug forms are redacted. The app shows the words and calls `confirm()` once the person stored them.
- **`db.account.removeMember(memberId)`** removes a lost device or an agent from every space the account administers, as one action on the account ([RFC 0010, "Losing a device"](0010-accounts.md#losing-a-device)).

**The Rust forms and what the calls return** (Carsten, 2026-11-12, NOC-273):

```rust
// On the new device or agent:
let db = Db::open(OpenOptions::new(lock).path("./data").pairing()).await?;
let invitation = db.invitation().await;

// On a device of the account:
let paired = db.account().pair(PairKind::Device, &invitation).await?;
// paired.answer: the PairingAnswer to show; paired.added: one (space, notice) per space the
// new device was added to; paired.ask_admins: the spaces whose admins the app asks.

// Back on the new device:
let notices = db.account().complete_pairing(&answer).await?;

// On a new device, recovering the account:
let db = Db::open(OpenOptions::new(lock).path("./data").recover(&words)).await?;

// Showing the recovery code until the person stored it:
if let Some(code) = db.account().recovery_code().await? {
    show(code.words());                          // twelve words; Display and Debug print "RecoveryCode(redacted)"
    code.confirm().await?;
}
```

- **`OpenOptions::pairing()` and `OpenOptions::recover(words)`** are the Rust forms of `pairing: "request"` and `recover`, builder methods as `agent()` is. `recover` takes the words as the person typed them, separated by white space and in any letter case; `Db::open` fails with `invalidValue` when they are not twelve words of the list or their checksum fails, and creates nothing. Not taken: a `Mode` enum passed to one method, which adds a type for two choices, and separate constructors such as `Db::recover`, which would duplicate every other option.
- **`db.account()`** returns an `Account`, a handle as `db.spaces()` and `db.sync()` are, with `pair`, `complete_pairing`, `remove_member` and `recovery_code`. In TypeScript the same calls stay on `db.account`.
- **`Account::pair(kind, &invitation)`** takes a `PairKind`, `PairKind::{Device, Agent}`, and returns a `Paired { answer: PairingAnswer, added: Vec<(SpaceId, Notice)>, ask_admins: Vec<SpaceId> }`: the answer to show, one notice per space the SDK added the new device to, and the spaces where no device of the account is an admin, whose admins the app asks ([RFC 0010, "Adding a device or an agent to an account"](0010-accounts.md#adding-a-device-or-an-agent-to-an-account)). For an agent, `added` and `ask_admins` are empty. In TypeScript `db.account.pair` returns the same three fields, `{ answer, added, askAdmins }`, rather than the answer alone, since the notices must reach the app. `PairKind` was taken over the existing `MemberKind`, which also holds `SubAgent`, a kind nobody pairs.
- **`PairingAnswer`** is public data with a text form, as `Invitation` is: `Display` gives it, `FromStr` reads it and fails with `invalidValue` ([RFC 0010, "The pairing answer"](0010-accounts.md#the-pairing-answer)). It carries the account key sealed to one device, which only that device opens, so it is not redacted.
- **`Account::complete_pairing(&answer)`** returns the notices of the pairing on this device: `DevicePaired` or `AgentPaired`, and `AccountLinked` when the database had an account of its own.
- **`Account::remove_member(member, &words)`** takes the recovery code, which RFC 0010's third step needs for the new account key, and returns a `Removed { removed: Vec<(SpaceId, Notice)>, ask_admins: Vec<SpaceId>, notice: Notice }`: the spaces it removed the member from, each with `MemberRemoved`'s notice, the spaces whose admins the app asks, and `AccountKeyRenewed`. In TypeScript, `db.account.removeMember(memberId, { recover: words })`. Not taken: removing without the code and renewing the account key in a second call, which would let an app stop after the first and leave the lost device's account key valid.
- **`RecoveryCode`** holds the code until the person stored it. `Display` and `Debug` print `RecoveryCode(redacted)`, it implements no serde trait, so no serializer can put it into a log, and `words()` gives the twelve words as `[&'static str; 12]`, the one way to read them. `confirm(self)` consumes it: the SDK forgets the code and keeps only the recovery key's public half ([RFC 0010, "The recovery code"](0010-accounts.md#the-recovery-code)). In TypeScript, `code.words()` is a method and `toString()`, `toJSON()` and Node's inspect give `"RecoveryCode(redacted)"`. A serde form that writes the redacted text was not taken: it would let a code pass through a serializer silently.
- **When the app gets it.** The first open of a database that is opened neither for pairing nor for recovery creates the account and its code. The SDK keeps the code in the keystore or key file, next to the account key, until `confirm()`. `db.account().recovery_code()` returns `Some(code)` on every open until then and `None` after, so an app that crashed before it showed the code shows it on the next open. Not taken: handing the code only as a return value of the open that created the account, which a crash loses for good; and a notice stream for it, which an app that does not follow it misses.

**The notices of accounts** (Carsten, 2026-11-12, NOC-273), as `NoticeCode` values with their texts:

| Code | Returned by | Text |
| --- | --- | --- |
| `DeviceAdded` | `pair`, once per space the new device joins | "Your new device was added to this space and reads its whole history." |
| `DevicePaired` | `complete_pairing` of a device | "This device now holds your account key. Keep your recovery code: it is the way back if you lose every device." |
| `AgentPaired` | `complete_pairing` of an agent | "This agent acts for your account in the spaces an admin adds it to. It holds no account key." |
| `AccountLinked` | `complete_pairing` of a database that had its own account | "Spaces this database joined before keep its old account. Other members see it as linked to yours." |
| `AccountRecovered` | `db.account().recovered()`, on the open that recovered the account (Carsten, 2026-11-13, NOC-289) | "Your account is back with a new account key. Your spaces come back when one of their members adds this device." |
| `AccountKeyRenewed` | `remove_member` | "Your account has a new account key. Members that have not learnt it yet still accept the old one." |

Reusing `MemberAdded` for the automatic adds was not taken: its text speaks of a member someone chose to add, not of the person's own device.

Until accounts are built, each database draws its own account key once, on its first open, keeps it next to the member key, and certifies its own member with it: as a device, or as an agent when the Rust SDK opens it with `agent()` ([The Rust SDK](#the-rust-sdk); [RFC 0003, "The account key"](0003-keys-and-cryptography.md#the-account-key)). A person's devices are unrelated accounts until RFC 0010 links them, so `members.add` asks for the fingerprint of each, as for any account new to the device. Two options were not taken: the app passing the account key, which puts a key in the app's hands and which principle 5 rules out; and no space until accounts exist, which blocks every space.

## Sync and status

```ts
const sync = db.sync.current();
// { connection: "connected", relay: "wss://relay.example.com", unconfirmed: 3, refused: 0, epochChanged: false }

const status = project.status.current();
// { role, initialSyncComplete, behind, gaps, parked, rejected, refused, lostGrants,
//   equivocations, exposed, waitingForKey, provisionalErasures }

const stop = db.sync.subscribe((sync) => showConnection(sync));
await db.sync.pause();
await db.sync.resume();
```

**The relay connection.** `nc.open({ relay })` names one relay for the database. Moving a space between relays is not designed ([RFC 0007, "What this RFC does not cover"](0007-relay.md#what-this-rfc-does-not-cover)); a relay per space can be added as an option of `db.spaces.create` later, and was not taken for now. The SDK opens one connection, authenticates with the member key, pulls, pushes and subscribes for every space, and reconnects with backoff when the connection drops ([RFC 0007, "Island mode"](0007-relay.md#island-mode)). The app never sees the protocol. `db.sync.pause()` and `resume()` stop and restart it, for an app that wants to stay offline.

**`db.sync`**, a report:

| Value | Meaning |
| --- | --- |
| `connection` | `"connected"`, `"connecting"`, `"offline"` (no relay reachable, the SDK retries), `"paused"` or `"none"` (opened without a relay). |
| `relay` | The relay `nc.open` named, absent without one. In a browser tab that reaches another tab's database, it is the owning tab's relay, which serves every tab ([RFC 0006, "The browser"](0006-local-view.md#the-browser)). TypeScript only (Carsten, 2026-11-13, #317). |
| `unconfirmed` | This member's changes the relay has not confirmed, as `db.durability` counts them ([RFC 0006, "What the SDK reports"](0006-local-view.md#what-the-sdk-reports)). |
| `refused` | Changes of this member the relay refused ([RFC 0002, "A change the relay refuses after the member folded it"](0002-change-and-log-format.md#a-change-the-relay-refuses-after-the-member-folded-it)). |
| `epochChanged` | The relay was restored and the SDK is pushing again what it lost ([RFC 0007, "Restores and the relay epoch"](0007-relay.md#restores-and-the-relay-epoch)). |
| `error` | The last error that stops syncing, such as `unsupported_version`, by its RFC 0007 code; never content. A code RFC 0007 does not list shows as `unknown_error`, so the app still learns that syncing stopped, and the relay's own text never reaches the app. |

In Rust the report is a `SyncReport`, and its values are a `SyncStatus`; a space's report, below, is a `SpaceStatusReport`, and its values are a `SpaceStatus` ([The Rust SDK](#the-rust-sdk)).

An unknown relay code is shown as `unknown_error` because a faulty or hostile relay could otherwise put any string where the app reads `error`. An empty `error` would not tell the app why syncing stopped, and keeping the previous `error` could show a stale reason; neither was taken.

**`space.status`**, a report per space, with everything a space can hold that an app does not see in a read:

| Value | Meaning |
| --- | --- |
| `role` | This member's role, or `"removed"`. A removed member keeps reading its copy. |
| `initialSyncComplete`, `behind` | Whether the first pull reached the head, and how many relay positions the member has heard of but not folded ([RFC 0006, "Reads never touch the network"](0006-local-view.md#reads-never-touch-the-network)). |
| `gaps` | Dependencies the member does not hold ([RFC 0002, "Dependencies"](0002-change-and-log-format.md#dependencies)). |
| `parked` | Per reason, the parked changes and operations, and the highest model version they need; "another model" apart ([RFC 0005, "Data from a newer model"](0005-schema-and-model-lock.md#data-from-a-newer-model)). |
| `rejected` | Per reason, the rejected changes ([RFC 0002, "Rejected changes"](0002-change-and-log-format.md#rejected-changes)). |
| `refused` | This member's changes in this space the relay refused, by change id and code. They stay folded on this member and do not reach members that sync only through the relay; the app may write a change that reverts one ([RFC 0002, "A change the relay refuses after the member folded it"](0002-change-and-log-format.md#a-change-the-relay-refuses-after-the-member-folded-it)). |
| `lostGrants`, `equivocations` | RFC 0008's lost grants, and members that wrote two changes under one sequence ([RFC 0008, "The cutoff"](0008-membership.md#the-cutoff)). |
| `exposed`, `waitingForKey` | Whether members write under a generation a removed member may hold, and whether this member waits for an envelope ([RFC 0008, "Two removals that rotate the key at once"](0008-membership.md#two-removals-that-rotate-the-key-at-once)). |
| `provisionalErasures` | The erasures of this space that are still provisional, each with its shred, its age on this device, whether this device's account may take it back (`canUndo`), the member that performed it (`performer`), and the notice `erasurePending`. A space whose members all belong to one account signs off its own erasures, which stay provisional for 7 days ([RFC 0008, "A provisional erasure in a space with one account"](0008-membership.md#a-provisional-erasure-in-a-space-with-one-account)). Every device of the account sees it, and an app must show it: it is how the account's owner notices, on another device, an erasure by a stolen one while it can still be taken back (Carsten, 2026-11-11, NOC-230). |

**Durability** is RFC 0006's `db.durability`, unchanged: `current()`, `subscribe(listener)` and `requestPersistent()`, with `storage`, `unconfirmed`, `oldestUnconfirmedAt` and `atRisk` ([RFC 0006, "What the SDK reports"](0006-local-view.md#what-the-sdk-reports)). It is on every target; natively `storage` is `"persistent"`.

**`db.view`**, a report of the view itself: whether a rebuild runs and how far it is ([RFC 0006, "What a rebuild costs"](0006-local-view.md#what-a-rebuild-costs)).

**Confirmation of one write.** `write` resolves on the device. An app that must know when the relay holds a change reads `db.sync.isConfirmed(result.change)` or waits for it with `db.sync.confirmed(result.change)`, a promise that resolves when the relay records a position for it. It may never resolve: the relay can refuse the change, or never be reached.

## Errors

The SDK throws one error class, `nc.Error`, with a string `code`, the ids it concerns in `ids`, each as text, and, for `unknownAccount`, the account's fingerprint in `fingerprint`, a plain string of the twelve blocks (Carsten, 2026-11-12, NOC-274). Codes are easy to match on in both SDKs, map one to one to Rust's `ErrorCode`, and keep the API surface small; a class per error was not taken. An error never carries content, a key or a personal field: "Decryption failed for change C in space S, generation 4" is the model ([RFC 0003, "Keys never reach logs, errors or telemetry"](0003-keys-and-cryptography.md#keys-never-reach-logs-errors-or-telemetry)).

| Code | Thrown by | When |
| --- | --- | --- |
| `lockOutOfDate` | `nc.open` | The model names a collection, field or index the lock does not have, or with another type. Run `ncdb model build`. |
| `modelRefused` | `nc.open` | The lock cannot be read or breaks RFC 0005's rules, has another `model.id` than the stored one, or is neither an additive successor of the newest lock nor an older lock the newest one descends from; lists each refused change ([RFC 0005, "Two checks: at build and on the device"](0005-schema-and-model-lock.md#two-checks-at-build-and-on-the-device)). |
| `spaceOfAnotherModel` | `db.spaces.get`, writes | The space's genesis names another `model.id`; names both ids. |
| `storageFull` | writes, `nc.open` | The disk or the browser's quota is full. |
| `storageFailed` | writes, reads, `nc.open`, `Db::open` | Any other storage failure, such as a damaged database file, or an existing database whose key file or keystore entry is gone or does not read as a key record ([The Rust SDK](#the-rust-sdk)). Freeing space would not help, so it is not `storageFull`, at the cost of one more code. |
| `keyStoreUnavailable` | `nc.open` in Node.js, `Db::open` | No platform keystore is reachable and the app named no key file. The message names the key-file option, `keyFile` or `key_file` ([Open and close](#open-and-close)). |
| `closed` | every call | The database was closed. |
| `notFound` | `space.update`, `delete`, `restore`, `db.spaces.get`, writes, queries | The copy holds no such object or space, or a call names a collection, field or index the lock does not have. TypeScript catches the latter when it compiles; Rust, which names them by text, cannot. |
| `personKeyNotHeld` | writes | A personal field's person has a person key this device does not hold yet; the app retries once sync has fetched it (Carsten, 2026-11-12, NOC-255; [Personal fields](#personal-fields)). |
| `invalidValue`, `deprecated`, `noPerson` | writes, queries; `invalidValue` also `nc.open` in Node.js and `Db::open` | See [Field operations per field type](#field-operations-per-field-type) and [Personal fields](#personal-fields). A query is `invalidValue` when it fixes another number of equality values than its index declares, asks for a page size of 0 or above the maximum, or uses an index of the other scope: `space.find` on an index without `scope: "space"`, or `db.find` on one with it. Opening is `invalidValue` when it names neither a path nor memory to keep the database in ([Open and close](#open-and-close)). |
| `unknownType` | writes | The field's plugin type is not registered in this member's core under the rule the lock names, as in every TypeScript member in v1; names the type id and the rule (Carsten, 2026-11-12, NOC-232; [RFC 0011, "A type that is not registered"](0011-plugin-field-types.md#a-type-that-is-not-registered)). |
| `notAllowed`, `notMember`, `lastAdmin` | writes, membership calls | The member's role does not allow it, it is no longer a member, or it is the last admin. |
| `unknownAccount` | `members.add` | The invitation's account is new to this device and no person confirmed its fingerprint; carries the fingerprint. |
| `invalidCertificate` | `members.add` | The invitation's certificate does not hold. It may mean a forged invitation. |
| `alreadyMember` | `members.add` | The invitation's member is a member of the space already. |
| `cursorOfAnotherModel` | `db.find` | The cursor was made under another model or index. |

What syncing finds is never thrown, because no call of the app caused it: a relay refusal, a rejected or parked change, a lost grant or a gap arrives in a report. They arrive while no app call runs, and they are states of the copy, not failures of a call, so reports keep them readable at any time.

## The Rust SDK

The Rust SDK, crate `nocentraldb`, has the same calls in snake case, async, and the same behaviour on every target ([GENESIS.md, principle 7](../../factory/GENESIS.md#principles-in-priority-order)). It reads the lock at build time or at run time; RFC 0005 says typed code can be generated from the lock later, so v1 reads and writes fields by name, with `Value`. That works for every lock today, and generated structs can be added without changing it; typed structs generated from the lock in v1 were not taken:

```rust
use nocentraldb::{Db, OpenOptions, Value};

let db = Db::open(OpenOptions::new(include_str!("../model.lock.json"))
    .path("./data")
    .relay("wss://relay.example.com")).await?;
let project = db.spaces().create().await?;

let result = project.write(|tx| {
    let task = tx.create("tasks", [("title", Value::from("Ship the SDK")), ("priority", Value::from(2))])?;
    tx.update("tasks", task, [("tags", Value::add(["docs"]))])?;
    Ok(())
}).await?;

let page = db.find(db.index("tasks", "byStatusPriority")?.equal(["open"]).take(50)).await?;
for task in &page.items {
    let title: Option<&str> = task.get("title")?;
}
let mut live = db.subscribe(db.index("tasks", "byStatusPriority")?.equal(["open"]).take(50));
while let Some(page) = live.next().await { /* ... */ }
```

- `Db::open(options)` takes the lock's text; there is no TypeScript model, so the `lockOutOfDate` check does not apply.
- **Clearing a field.** `Value::clear()`, a constructor beside `Value::add` and `Value::remove`, clears a field as `null` does in TypeScript: `tx.update("tasks", id, [("due", Value::clear())])` becomes a set (1) with no value ([Field operations per field type](#field-operations-per-field-type)). It clears a basic value, a link or a reference; a set or a list of links has nothing to clear in one value, so a clear of one fails with `invalidValue` (Carsten, 2026-11-12, NOC-277). `Option<Value>` in every write was not taken: it would make every value an `Option` to clear the few fields an app ever clears.
- **Where the database lives.** `path(dir)` names a directory the SDK owns: the SDK creates it, and it holds one SQLite file, `nocentraldb.sqlite`, which leaves room for more files later; a `path` that names the SQLite file itself was not taken. The SDK keeps the member key in the platform keystore ([RFC 0003, "The member key"](0003-keys-and-cryptography.md#the-member-key)), in one entry per database: the service `nocentraldb`, whose user is the canonical absolute path of the database directory. It holds the key record RFC 0003 defines. One entry per database leaves nothing to keep in step; a directory that is moved or renamed loses the link to its entry, and opening it then fails rather than draw a new member key silently. A random id stored in the database as the user would survive a move, but a copied database then shares the keystore entry with its original. `key_file(path)` keeps the keys in that file instead, for a headless agent that has no keystore; only the file's permissions protect it. `in_memory()` keeps the database and its keys in memory, for tests: they are gone when the `Db` is dropped. This works the same for a desktop app, a server and an agent in a container, and only an app that asks for a key file gets one. The platform's app data directory, found from an app id, was not taken: it suits apps but rarely servers and agents, and can come later on top; nor was keeping native data in memory only. `Db::open` with neither `path(dir)` nor `in_memory()` is refused with `invalidValue`; with no platform keystore reachable and no `key_file` given, it fails with `keyStoreUnavailable`, whose message names `key_file`, and creates nothing on disk ([Open and close](#open-and-close)). An existing database whose key file or keystore entry is gone, or does not read as a key record, fails with `storageFailed` and draws no new keys: new keys would hold none of the space keys of its spaces, and every space would turn `notFound`. Drawing new keys was not taken, because the database would open but none of its spaces can be read; nor was `keyStoreUnavailable`, whose message names a key file, which does not help a database whose keys were in the keystore.
- **The account key and the member's kind.** Until accounts are built, each database draws its own account key once and keeps it next to the member key ([Accounts and devices](#accounts-and-devices); [RFC 0003, "The account key"](0003-keys-and-cryptography.md#the-account-key)). `agent()` marks the member as an agent in its certificate; without it, the member is a device.
- **The Rust names.** Besides `Db`, `Space`, `Spaces`, `Transaction`, `Query`, `Page` and `Cursor`, which follow [Terms](#terms), the SDK has `ObjectId`, `SpaceId`, `ChangeId` and `MemberId`, which print and parse as lowercase hex, a member id as 64 digits (Carsten, 2026-11-13, NOC-298); `Object`, a read object, whose `id()`, `space()` and `deleted()` stand for `$id`, `$space` and `$deleted`, and whose `get::<T>(name)` reads a field; `Written`, what `write` returns, with `change` and `ids`; and `page.has_more` for `hasMore`. The names follow the TypeScript names in snake case; `$` cannot start a Rust name, so `$id`, `$space` and `$deleted` become methods.
- **Spaces and their list.** `db.spaces().list()` returns `Vec<SpaceInfo>`, one per space of the member. `SpaceInfo { id: SpaceId, role: Option<Role>, model: SpaceModel }` gives the member's role in the space, `None` once the member was removed or while no membership of it has been folded yet, and whether the space is of the lock's model ([Spaces](#spaces)). Roles are `Role::{Reader, Writer, Admin}`, the roles of [Invitations and members](#invitations-and-members), and the model is `SpaceModel::{Ours, Another}`, for `"ours"` and `"another"`. Other names, such as `SpaceSummary` or `ListedSpace` for `SpaceInfo`, or `ModelOf` for `SpaceModel`, were not taken. `Spaces::subscribe() -> impl Stream<Item = Vec<SpaceInfo>>` follows the list as [Spaces](#spaces) describes: the current list at once, then each new list when a space is added, left or removed, or its role changes (Carsten, 2026-11-12, NOC-241).
- **Invitations and members.** `db.invitation()` returns an `Invitation`, which holds the member id, its two public keys, its certificate and its account's fingerprint, a `Fingerprint`, which `invitation.fingerprint()` gives; no key bytes reach the app ([Invitations and members](#invitations-and-members)). A `Fingerprint` displays as RFC 0010's 60 digits in twelve blocks of five ([RFC 0010, "The fingerprint"](0010-accounts.md#the-fingerprint)). `Invitation` implements `Display`, which writes its `ncdb-invite-1:` text form, and `FromStr`, so `text.parse::<Invitation>()` reads one and fails with `ErrorCode::InvalidValue` on a text that does not follow [RFC 0003, "Invitation"](0003-keys-and-cryptography.md#invitation); a function of its own, such as `Invitation::parse`, was not taken, since `FromStr` is how Rust reads a value from text. `space.members()` returns `Members`, a handle like `db.spaces()`. `Members::add(&Invitation, AddMember) -> Result<Notice, Error>` adds a member and returns its notice, `Notice { code: NoticeCode, text: &'static str }`, with codes such as `NoticeCode::MemberAdded`; `remove`, `set_role` and the other calls that change who can read return the same type, as in TypeScript. `Members::list() -> Result<Vec<MemberInfo>, Error>` gives the current members, each a `MemberInfo { id: MemberId, account: Fingerprint, linked_to: Option<Fingerprint>, kind: MemberKind, role: Option<Role>, is_self: bool }`, `is_self` because `self` is a Rust keyword, and `MemberKind` is `Device`, `Agent` or `SubAgent`, `#[non_exhaustive]` (Carsten, 2026-11-11, NOC-203). `account` is the account the member's certificate names, which a membership written before that account was linked into another keeps ([RFC 0010, "Linking accounts drawn per database"](0010-accounts.md#linking-accounts-drawn-per-database)); `linked_to` is the fingerprint of the account it is linked into, when this database knows the link from its own account's link statements, and `None` otherwise, so that the app can show "linked to" next to the old fingerprint. `MemberInfo` is `#[non_exhaustive]`, so that a later field is no breaking change (Carsten, 2026-11-13, NOC-307 A1). `account` showing the account it is linked into, with the old one in a field of its own, was not taken: an app that compared `account` with a fingerprint a person checked would see it change. Nor was a separate `db.account().linked()` call, which covers only the database's own account. `MemberId` is public and opaque: `Copy`, `Eq`, `Ord`, `Hash` and `Debug`, with no text form yet. `Members::remove(MemberId) -> Result<Notice, Error>` returns `NoticeCode::MemberRemoved`, and `Members::set_role(MemberId, Role) -> Result<Notice, Error>` returns `NoticeCode::RoleSet`. An `&Invitation` as the argument of `remove` and `set_role` was not taken, since an admin rarely still holds the invitation of an existing member. `AddMember { role: Role, fingerprint: Option<Fingerprint> }` is a plain struct: without the fingerprint, a member of an account new to the database fails with `ErrorCode::UnknownAccount`, and `Error::fingerprint() -> Option<&Fingerprint>` gives the fingerprint to show. A builder, such as `members().add(&invitation).role(Role::Writer).fingerprint(fingerprint)`, was not taken.
- **The role is required in Rust.** `AddMember.role` is a `Role`, not an `Option<Role>`, so an app passes `Role::Writer` for an agent, where TypeScript lets it leave the role out. An `Option<Role>`, with `None` meaning a writer for an agent and an error for a device, was not taken.
- **Erasures.** `space.erasures()` returns `Erasures`, whose `list()` returns `Vec<Erasure>`, the audit of [Erasures](#erasures) in Rust: `Erasure { shred: ChangeId, state: ErasureState, performer: MemberId, performer_account: Fingerprint, sign_off: Fingerprint, members: Vec<ErasureMember> }`, with `ErasureState::{Final, Provisional, TakenBack}`, `ErasureMember { id: MemberId, account: Fingerprint, current: bool, acknowledged: Acknowledged }` and `Acknowledged::{Deleted, Seen, Missing}`. The structs and both enums are `#[non_exhaustive]`, so a value added later breaks no app (Carsten, 2026-11-12, NOC-246).
- `space.write(|tx| ...)` takes a closure that records; it is synchronous for the same reason as in TypeScript.
- `subscribe` returns a `Stream`; dropping it stops it.
- **The sync report.** `db.sync()` returns a `SyncReport`, whose `current()` gives a `SyncStatus` with `connection`, `unconfirmed`, `refused`, `epoch_changed` and `error`, and whose `subscribe()` follows it. `connection` is one of `Connection::Connected`, `Connecting`, `Offline`, `Paused` or `None`, the values of [Sync and status](#sync-and-status). The report is not named `Sync`, which would shadow Rust's `Sync` marker trait. An unknown relay code is `unknown_error` here too ([Sync and status](#sync-and-status)).
- **The space status.** `space.status()` returns a `SpaceStatusReport`, whose `current()` gives a `SpaceStatus` and whose `subscribe()` follows it, as `db.sync()` does with its `SyncReport`. `SpaceStatus` is `#[non_exhaustive]`, so a value added later breaks no app, and it holds the twelve values of [Sync and status](#sync-and-status) in snake case: `role: Option<Role>`, where `None` means removed or nothing folded yet, as in `SpaceInfo`; `initial_sync_complete: bool`; `behind: u64`; `gaps: Vec<ChangeId>`, the missing dependencies; `parked: BTreeMap<ParkReason, u64>` and `rejected: BTreeMap<RejectReason, u64>`, each reason a `#[non_exhaustive]` enum that names the RFCs' reasons in Rust case: `ParkReason` holds RFC 0002's `UnknownSuite`, `UnknownKind`, `UnknownFormat`, `UnknownPayloadFormat` and `AnotherModel`, another model as a reason of its own ([RFC 0002, "What a member does with a change it receives"](0002-change-and-log-format.md#what-a-member-does-with-a-change-it-receives)), RFC 0005's `UnknownCollection`, `UnknownField` and `UnknownType`, the last for a field whose plugin type and rule the app did not register (Carsten, 2026-11-12, NOC-232; [RFC 0011, "A type that is not registered"](0011-plugin-field-types.md#a-type-that-is-not-registered)), RFC 0004's `UnknownOperation`, `UnknownArgument` and `MalformedArguments` ([RFC 0004, "Operations a member does not understand"](0004-merge-rules-basic-types.md#operations-a-member-does-not-understand)), and `UnknownKey` for an operation that carries an unknown "must understand" key, which RFC 0002's step 7 lists without naming it (Carsten, 2026-11-11, NOC-199), and `RejectReason` holds RFC 0002's `UnknownAuthor`, `BadSignature`, `NotMember`, `NoKey` and `Undecryptable`; `refused: Vec<Refused>`, with `Refused { change: ChangeId, code: String }`, whose code is one of RFC 0007's codes or `unknown_error`, never the relay's own text; `lost_grants: Vec<ChangeId>`, the lost grant changes; `equivocations: Vec<ChangeId>`, the changes that share an author and sequence; `exposed: bool`; and `waiting_for_key: bool`; and `provisional_erasures: Vec<ProvisionalErasure>`, with `ProvisionalErasure { change: ChangeId, age: Duration, can_undo: bool, performer: MemberId, notice: Notice }`, the provisional erasures of a space with one account, whose age counts from when this device first held the shred and whose `can_undo` is true on a device of the performer's account that is an admin of the space; `performer` names the member that wrote the shred, so an app can show which device erased, from `members().list()`, and `notice` is the notice `NoticeCode::ErasurePending` with the text "Personal data was erased on a device of this account. The erasure becomes final after 7 days; until then an admin of this account can take it back.", which the SDK gives on every device of the account (`performer` and the text: pm, 2026-11-12, NOC-234; Carsten, 2026-11-11, NOC-230; [RFC 0008, "A provisional erasure in a space with one account"](0008-membership.md#a-provisional-erasure-in-a-space-with-one-account)). Any other shred is final when accepted, so no other shred waits and the status lists none (Carsten, 2026-11-11, NOC-224; [RFC 0008, "A shred is final when accepted"](0008-membership.md#a-shred-is-final-when-accepted)). Next to them, `newest_model: Option<u32>` gives the highest model version the parked changes need. Next to them as well, `bad_person_key_records: Vec<Fingerprint>` names the accounts whose person-key records failed their checks when the member last fetched them: their writer signed them, but they do not unwrap under their generation or their key does not match the commitment in the log, so the member ignores them ([RFC 0003, "The person key"](0003-keys-and-cryptography.md#the-person-key)); its TypeScript form comes with the TypeScript status (Carsten, 2026-11-11, NOC-222). The status names no member by its `MemberId`: change ids and fingerprints carry what it needs (Carsten, 2026-11-11, NOC-193); `MemberId` belongs to the member calls ([Invitations and members](#invitations-and-members) above). Not taken: `StatusReport` as the report's name; an operation's unknown "must understand" key counted as `UnknownArgument`, which names a key of the operation's arguments, not of the operation; a `SpaceRole` enum that tells "removed" apart from "nothing folded yet"; reasons as text codes, as `SyncStatus` gives its `error`, which the compiler would not check; `gaps` as a count; and a `LostGrant` with the member and the change, which the status does not need.
- **The view's report.** `db.view()` returns a `ViewReport`, whose `current()` gives a `ViewStatus` and whose `subscribe()` follows it, as `db.sync()` does with its `SyncReport`. `ViewStatus { rebuild: Option<RebuildProgress> }` is `None` while no rebuild runs. `RebuildProgress { folded: u64, total: u64 }` counts frames of the log, and its total grows while changes arrive during the rebuild. Both are `#[non_exhaustive]` (Carsten, 2026-11-11, NOC-227). The rebuild folds into staging tables beside the view, whose names start with `r_`, and keeps no marker of how far it is: a rebuild cut off by a close starts again at the next open ([RFC 0006, "How"](0006-local-view.md#how)) (Carsten, 2026-11-11, NOC-227). Not taken: a percentage, which hides how large the log is; `rebuilding: bool` alone, which shows no progress; and a marker in `meta` that would resume a cut-off rebuild, which would add to the database's format while the old view stays valid until the swap anyway.
- **Personal fields.** A personal field is written and read by name, as any field. `tx.create_with(collection, values, Create { same_person_as: Some(object) })` creates an object of the same person as `object`, read from another space, beside `tx.create`, which takes no options; `Create { same_person_as: Option<Object> }` is a plain struct, as `AddMember` is. A personal field whose person link is unset is refused with `ErrorCode::NoPerson`, and one whose person has a person key this device does not hold yet with `ErrorCode::PersonKeyNotHeld` (Carsten, 2026-11-12, NOC-255). `object.erased()` gives the names of the object's erased fields, as `$erased` does (Carsten, 2026-11-12, NOC-244; [Personal fields](#personal-fields)). A builder, such as `tx.create(collection, values).same_person_as(object)`, was not taken, as for `members().add`.
- Errors are `nocentraldb::Error` with an `ErrorCode` enum whose variants are the codes above in Rust case, such as `ErrorCode::LockOutOfDate`. `Debug` of every type that holds content or a cursor prints `[redacted]` ([RFC 0003, "Keys never reach logs, errors or telemetry"](0003-keys-and-cryptography.md#keys-never-reach-logs-errors-or-telemetry)).

## Public names

Every public name this RFC defines or uses, in one place, each with the RFC it builds on. "Named in" marks a name another RFC named already; this RFC only uses it.

**Open and close** (`@nocentraldb/sdk`):

| Name | What it is | Builds on |
| --- | --- | --- |
| `nc.open` | Opens this member's database; options `model`, `lock`, `relay`, `name` (in a browser; Carsten, 2026-11-12, NOC-274), `persistentStorage` (`"on-open"`, the default this RFC names, or `"on-request"`), `warnings`, `subAgent`, `pairing` (`"request"`), `recover`, and in Node.js only `path`, `keyFile`, `inMemory`, `agent` | Named in RFC 0006 with `model`, `persistentStorage`, `warnings`: [RFC 0006, "What the SDK reports"](0006-local-view.md#what-the-sdk-reports); `lock`: [RFC 0005](0005-schema-and-model-lock.md#two-checks-at-build-and-on-the-device); `pairing`, `recover`: [RFC 0010, "Adding a device or an agent to an account"](0010-accounts.md#adding-a-device-or-an-agent-to-an-account); `path`, `keyFile`, `inMemory`, `agent`: [RFC 0003, "The member key"](0003-keys-and-cryptography.md#the-member-key) |
| `db.close` | Closes the database | [RFC 0006, "One writer, many readers"](0006-local-view.md#one-writer-many-readers) |
| `nc.Error` | The one error class, with `code`, `ids` and `fingerprint` (Carsten, 2026-11-12, NOC-274) | [RFC 0003, "Keys never reach logs, errors or telemetry"](0003-keys-and-cryptography.md#keys-never-reach-logs-errors-or-telemetry) |
| `Db`, `Space`, `Tx`, `Invitation`, `Page<T>`, `Query<T>`, `Notice`, `SyncStatus`, `SpaceStatus`, `Durability` | The TypeScript types of a database, a space, a transaction, an invitation, a page and a query of `T`, a notice, and the three reports (Carsten, 2026-11-12, NOC-274) | `Invitation`: [RFC 0003, "Invitation"](0003-keys-and-cryptography.md#invitation); `Durability`: [RFC 0006, "What the SDK reports"](0006-local-view.md#what-the-sdk-reports) |

**Write:**

| Name | What it is | Builds on |
| --- | --- | --- |
| `space.write` | Records a transaction; one change | [RFC 0002, "The atomic unit"](0002-change-and-log-format.md#the-atomic-unit) |
| `tx.create` | Creates an object; option `samePersonAs` | [RFC 0004, "Creating and deleting an object"](0004-merge-rules-basic-types.md#creating-and-deleting-an-object); [RFC 0005, "Identifying the person"](0005-schema-and-model-lock.md#identifying-the-person) |
| `tx.update` | Writes fields of an object | [RFC 0004, "The arguments of each operation"](0004-merge-rules-basic-types.md#the-arguments-of-each-operation) |
| `tx.delete` | Deletes an object | [RFC 0004, "Creating and deleting an object"](0004-merge-rules-basic-types.md#creating-and-deleting-an-object) |
| `tx.restore` | Restores a deleted object | [RFC 0004, "Creating and deleting an object"](0004-merge-rules-basic-types.md#creating-and-deleting-an-object) |
| `space.create`, `space.update`, `space.delete`, `space.restore` | One-call shorthands of `space.write` | [RFC 0002, "The atomic unit"](0002-change-and-log-format.md#the-atomic-unit) |
| `nc.add`, `nc.remove` | Field operations on sets and links | [RFC 0004, "Sets"](0004-merge-rules-basic-types.md#sets) |
| `change`, `ids` | What `write` returns | [RFC 0002, "Change ids"](0002-change-and-log-format.md#change-ids) |

**Read and query:**

| Name | What it is | Builds on |
| --- | --- | --- |
| `space.get` | Reads one object | [RFC 0005, "Types for the app"](0005-schema-and-model-lock.md#types-for-the-app) |
| `space.inspect` | Reads one object with conflicts, lost removals and link statuses | [RFC 0004, "Late changes stay identifiable"](0004-merge-rules-basic-types.md#late-changes-stay-identifiable) |
| `space.outcome` | The outcome of one change on one field | [RFC 0004, "Late changes stay identifiable"](0004-merge-rules-basic-types.md#late-changes-stay-identifiable) |
| `value`, `conflicts`, `elements`, `lostRemovals`, `status`, `raw`, `author`, `stamp` | The parts of `inspect` | [RFC 0004, "Late changes stay identifiable"](0004-merge-rules-basic-types.md#late-changes-stay-identifiable) |
| `"ok"`, `"deleted"`, `"missing"`, `"wrongCollection"` | Link statuses | [RFC 0004, "Resolution"](0004-merge-rules-basic-types.md#resolution) |
| `"ok"`, `"notAccessible"`, `"missing"` | Reference states | [RFC 0005, "References into another space"](0005-schema-and-model-lock.md#references-into-another-space) |
| `"won"`, `"lost"`, `"replaced"`, `"applied"` | Outcomes | [RFC 0004, "Late changes stay identifiable"](0004-merge-rules-basic-types.md#late-changes-stay-identifiable) |
| `nc.Unknown` | A choice option the lock does not list: `{ unknown }` | [RFC 0004, "Value rules per kind"](0004-merge-rules-basic-types.md#value-rules-per-kind) |
| `$id`, `$space`, `$deleted`, `$conflicts`, `$raw`, `$erased` | What a read object carries beyond its fields; `$erased`: (Carsten, 2026-11-12, NOC-244) | [RFC 0005, "Names"](0005-schema-and-model-lock.md#names) |
| `db.find` | Runs a query across the member's spaces | Named in [RFC 0001, "The design in one page"](0001-nocentraldb.md#the-design-in-one-page) |
| `space.find` | Runs a query of an index scoped to one space | [RFC 0006, "Declared indexes"](0006-local-view.md#declared-indexes) |
| `model.<collection>.indexes.<index>` | The handle of a declared index | [RFC 0005, "Indexes"](0005-schema-and-model-lock.md#indexes) |
| `.equal(` | Fixes the index's equality prefix | [RFC 0006, "Declared indexes"](0006-local-view.md#declared-indexes) |
| `.take(` | The page size | Named in [RFC 0001, "The design in one page"](0001-nocentraldb.md#the-design-in-one-page) |
| `.after(` | Starts after a cursor | Named in [RFC 0001, "The design in one page"](0001-nocentraldb.md#the-design-in-one-page) |
| `items`, `cursor`, `hasMore` | The parts of a page | [RFC 0006, "Ordered pages"](0006-local-view.md#ordered-pages) |
| `nc.index` | Declares an index; this RFC adds the option `scope` | Named in [RFC 0005, "Public names"](0005-schema-and-model-lock.md#public-names) |
| `scope: "space"` | Index option: the index's first column is the space id | [RFC 0005, "Indexes"](0005-schema-and-model-lock.md#indexes) |

**Subscribe:**

| Name | What it is | Builds on |
| --- | --- | --- |
| `db.subscribe` | Follows a query's page | [RFC 0006, "Folding"](0006-local-view.md#folding) |
| `space.subscribe` | Follows a query in one space, or one object | [RFC 0006, "Folding"](0006-local-view.md#folding) |
| `db.changes.subscribe` | Ids touched by each fold, and `origin` | [RFC 0006, "Folding"](0006-local-view.md#folding) |

**Spaces and members:**

| Name | What it is | Builds on |
| --- | --- | --- |
| `db.spaces.create` | Creates a space: its genesis | [RFC 0008, "The first change of a space"](0008-membership.md#the-first-change-of-a-space); sketched in [RFC 0003](0003-keys-and-cryptography.md#how-the-sdk-hides-keys) |
| `db.spaces.get`, `db.spaces.list` | A space handle; every space of the member | [RFC 0006, "Where the view lives"](0006-local-view.md#where-the-view-lives) |
| `db.spaces.subscribe` | The space list as a stream: at once, then on every change (Carsten, 2026-11-12, NOC-241) | [RFC 0006, "Where the view lives"](0006-local-view.md#where-the-view-lives) |
| `db.spaces.createAdHoc` | An ad-hoc space for a sub-agent; options `copy`, `personal`, `local` | [RFC 0003, "Sub-agents get an ad-hoc space"](0003-keys-and-cryptography.md#sub-agents-get-an-ad-hoc-space) |
| `space.id` | The space id | [RFC 0008, "What it names"](0008-membership.md#what-it-names) |
| `space.leave` | Removes this member from the space | [RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights) |
| `space.discard` | Deletes a local ad-hoc space | [RFC 0003, "Sub-agents get an ad-hoc space"](0003-keys-and-cryptography.md#sub-agents-get-an-ad-hoc-space) |
| `db.invitation`, `nc.invitation` | This member's invitation; parses one from text | [RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys); [RFC 0003, "Invitation"](0003-keys-and-cryptography.md#invitation) |
| `space.members.add` | Adds members by invitation; options `role`, `fingerprint` | Sketched in [RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys) |
| `space.members.remove` | Removes a member and rotates the key | Sketched in [RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys) |
| `space.members.setRole` | Raises or lowers a role | [RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights) |
| `space.members.list` | The members, with `id`, `account`, `kind`, `role`, `self` | [RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights) |
| `space.erasures.list` | The audit of the space's erasures: `shred`, `state`, `performer`, `signOff`, and per member `id`, `account`, `current`, `acknowledged` (Carsten, 2026-11-12, NOC-246) | [RFC 0008, "Acknowledgement"](0008-membership.md#acknowledgement) |
| `"final"`, `"provisional"`, `"takenBack"` | States of an erasure | [RFC 0008, "A provisional erasure in a space with one account"](0008-membership.md#a-provisional-erasure-in-a-space-with-one-account) |
| `"deleted"`, `"seen"`, `"missing"` | What a member acknowledged of an erasure | [RFC 0008, "Acknowledgement"](0008-membership.md#acknowledgement) |
| `"reader"`, `"writer"`, `"admin"` | Roles | [RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights) |
| `"device"`, `"agent"`, `"subAgent"` | Kinds of member | [RFC 0003, "The account key"](0003-keys-and-cryptography.md#the-account-key) |
| `notice` | What a membership call does not do: `{ code, text }` | [RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys) |
| `memberAdded`, `memberRemoved`, `roleSet`, `spaceLeft`, `adHocSpaceCreated`, `erased` | Notice codes, each named by the call that returns it | [RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys) |
| `erasurePending` | The notice of a provisional erasure, which the space status gives on every device of the account, not a call (Carsten, 2026-11-11, NOC-230), with the text "Personal data was erased on a device of this account. The erasure becomes final after 7 days; until then an admin of this account can take it back." (pm, 2026-11-12, NOC-234) | [RFC 0008, "A provisional erasure in a space with one account"](0008-membership.md#a-provisional-erasure-in-a-space-with-one-account) |
| `db.account.pair`, `db.account.completePairing`, `db.account.removeMember` | Pairing a device or an agent, and removing a lost one | [RFC 0010, "Adding a device or an agent to an account"](0010-accounts.md#adding-a-device-or-an-agent-to-an-account) |
| `confirm` | Confirms that the person stored the recovery code | [RFC 0010, "The recovery code"](0010-accounts.md#the-recovery-code) |
| `db.account().recovered()` | `AccountRecovered` on the open that recovered the account, else `None`; the open returns a `Db`, so a method gives the notice (Carsten, 2026-11-13, NOC-289) | [RFC 0010, "The recovery code"](0010-accounts.md#the-recovery-code) |

**Sync and status:**

| Name | What it is | Builds on |
| --- | --- | --- |
| `db.sync.current`, `db.sync.subscribe` | The relay connection, as a report | [RFC 0007, "Island mode"](0007-relay.md#island-mode) |
| `db.sync.pause`, `db.sync.resume` | Stop and restart syncing | [RFC 0007, "Island mode"](0007-relay.md#island-mode) |
| `db.sync.isConfirmed`, `db.sync.confirmed` | Whether, and when, the relay holds a change | [RFC 0007, "Push: append signed, encrypted changes"](0007-relay.md#push-append-signed-encrypted-changes) |
| `space.status.current`, `space.status.subscribe` | What a space holds and a read does not show | [RFC 0002, "Rejected changes"](0002-change-and-log-format.md#rejected-changes); [RFC 0005, "Data from a newer model"](0005-schema-and-model-lock.md#data-from-a-newer-model) |
| `db.durability.current`, `db.durability.subscribe`, `db.durability.requestPersistent` | Durability of the copy | Named in [RFC 0006, "What the SDK reports"](0006-local-view.md#what-the-sdk-reports) |
| `db.view.current`, `db.view.subscribe` | Progress of a rebuild | [RFC 0006, "What a rebuild costs"](0006-local-view.md#what-a-rebuild-costs) |

**Errors:**

| Name | What it is | Builds on |
| --- | --- | --- |
| `lockOutOfDate`, `modelRefused`, `spaceOfAnotherModel` | Model errors | [RFC 0005, "Two checks: at build and on the device"](0005-schema-and-model-lock.md#two-checks-at-build-and-on-the-device) |
| `storageFull`, `storageFailed`, `closed`, `notFound`, `cursorOfAnotherModel` | Errors of the view | [RFC 0006, "Ordered pages"](0006-local-view.md#ordered-pages) |
| `keyStoreUnavailable` | Error of opening: no platform keystore and no key file | [RFC 0003, "The member key"](0003-keys-and-cryptography.md#the-member-key) |
| `invalidValue`, `deprecated`, `noPerson`, `personKeyNotHeld` | Errors of a write | [RFC 0005, "Data from a newer model"](0005-schema-and-model-lock.md#data-from-a-newer-model) |
| `unknownType` | Error of a write to a plugin field whose type is not registered | [RFC 0011, "A type that is not registered"](0011-plugin-field-types.md#a-type-that-is-not-registered) |
| `notAllowed`, `notMember`, `lastAdmin`, `unknownAccount`, `invalidCertificate`, `alreadyMember` | Errors of rights and membership | [RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights) |
| `unknown_error` | The sync report's `error` for a relay code RFC 0007 does not list | [RFC 0007, "The errors"](0007-relay.md#the-errors) |

**Rust** (`nocentraldb`): every name above in snake case, and:

| Name | What it is | Builds on |
| --- | --- | --- |
| `Db::open`, `OpenOptions` | Opens the database from the lock's text | [RFC 0005, "Two checks: at build and on the device"](0005-schema-and-model-lock.md#two-checks-at-build-and-on-the-device) |
| `OpenOptions::path`, `OpenOptions::in_memory` | Where the database lives: a directory, or memory for tests | [RFC 0006, "Where the view lives"](0006-local-view.md#where-the-view-lives) |
| `OpenOptions::key_file` | A key file for a headless agent without a keystore | [RFC 0003, "The member key"](0003-keys-and-cryptography.md#the-member-key) |
| `Db::sync`, `SyncReport` | The relay connection, as a report: `current()` and `subscribe()` | [RFC 0007, "Island mode"](0007-relay.md#island-mode) |
| `SyncStatus` | What `current()` gives: `connection`, `unconfirmed`, `refused`, `epoch_changed`, `error` | [RFC 0007, "Island mode"](0007-relay.md#island-mode) |
| `Connection` | `Connected`, `Connecting`, `Offline`, `Paused`, `None` | [RFC 0007, "Island mode"](0007-relay.md#island-mode) |
| `Space::status`, `SpaceStatusReport` | A space's status, as a report: `current()` and `subscribe()` | [RFC 0002, "Rejected changes"](0002-change-and-log-format.md#rejected-changes); [RFC 0008, "The cutoff"](0008-membership.md#the-cutoff) |
| `SpaceStatus` | What `current()` gives: the twelve values of `space.status` in snake case, `newest_model` and `bad_person_key_records` | [RFC 0002, "Rejected changes"](0002-change-and-log-format.md#rejected-changes); [RFC 0008, "The cutoff"](0008-membership.md#the-cutoff) |
| `ParkReason`, `RejectReason` | Why a change or operation is parked or rejected, in Rust case | [RFC 0002, "Rejected changes"](0002-change-and-log-format.md#rejected-changes); [RFC 0005, "Data from a newer model"](0005-schema-and-model-lock.md#data-from-a-newer-model) |
| `ProvisionalErasure` | A provisional erasure of a space with one account: `change`, `age`, `can_undo`, `performer` and `notice`, the notice `NoticeCode::ErasurePending` | [RFC 0008, "A provisional erasure in a space with one account"](0008-membership.md#a-provisional-erasure-in-a-space-with-one-account) |
| `Refused` | A change of this member the relay refused: `change` and `code` | [RFC 0007, "The errors"](0007-relay.md#the-errors) |
| `Db::view`, `ViewReport` | The view's own report: `current()` and `subscribe()` | [RFC 0006, "What a rebuild costs"](0006-local-view.md#what-a-rebuild-costs) |
| `ViewStatus`, `RebuildProgress` | Whether a rebuild of the view runs, and how far it is: `folded` of `total` frames | [RFC 0006, "What a rebuild costs"](0006-local-view.md#what-a-rebuild-costs) |
| `OpenOptions::plugin` | Hands a plugin's field types to the core; called once per plugin | [RFC 0011, "Handing plugins to the core"](0011-plugin-field-types.md#handing-plugins-to-the-core) |
| `Data` | A plugin field's read value and write intent, read with `get::<Data>` and written with `Value::from` | [RFC 0011, "The data a plugin reads and writes"](0011-plugin-field-types.md#the-data-a-plugin-reads-and-writes) |
| `OpenOptions::agent` | Certifies the member as an agent | [RFC 0003, "The account key"](0003-keys-and-cryptography.md#the-account-key) |
| `Space`, `Spaces` | A space handle; what `db.spaces()` returns | [RFC 0008, "What it names"](0008-membership.md#what-it-names) |
| `Spaces::list`, `SpaceInfo` | Every space of the member: `id`, `role`, `model` | [RFC 0006, "Where the view lives"](0006-local-view.md#where-the-view-lives) |
| `Spaces::subscribe` | The space list as a stream of `Vec<SpaceInfo>` | [RFC 0006, "Where the view lives"](0006-local-view.md#where-the-view-lives) |
| `SpaceModel` | `Ours`, `Another`: whether a space is of the lock's model | [RFC 0005, "Two checks: at build and on the device"](0005-schema-and-model-lock.md#two-checks-at-build-and-on-the-device) |
| `Role` | `Reader`, `Writer`, `Admin` | [RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights) |
| `Db::invitation`, `Invitation`, `Fingerprint` | This member's invitation, written with `Display` and read with `FromStr` as RFC 0003's text form, and its account's fingerprint, shown as 60 digits | [RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys); [RFC 0010, "The fingerprint"](0010-accounts.md#the-fingerprint) |
| `Space::members`, `Members`, `Members::add`, `AddMember` | The members of a space; adds one by invitation, with `role` and `fingerprint` | [RFC 0003, "Sharing a space key with a new member"](0003-keys-and-cryptography.md#sharing-a-space-key-with-a-new-member) |
| `Members::remove`, `Members::set_role` | Removes a member; raises or lowers its role. Both take a `MemberId` and return a `Notice` | [RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights) |
| `Members::list`, `MemberInfo`, `MemberId`, `MemberKind` | The current members: `id`, `account`, `linked_to`, `kind`, `role`, `is_self`; the member id, 64 lowercase hex digits; `Device`, `Agent`, `SubAgent` | [RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights) |
| `Space::erasures`, `Erasures`, `Erasures::list` | The audit of a space's erasures, as `Vec<Erasure>` | [RFC 0008, "Acknowledgement"](0008-membership.md#acknowledgement) |
| `Erasure`, `ErasureState` | One erasure: `shred`, `state`, `performer`, `performer_account`, `sign_off`, `members`; `Final`, `Provisional`, `TakenBack` | [RFC 0008, "Acknowledgement"](0008-membership.md#acknowledgement) |
| `ErasureMember`, `Acknowledged` | A member of an erasure's audit: `id`, `account`, `current`, `acknowledged`; `Deleted`, `Seen`, `Missing` | [RFC 0008, "Acknowledgement"](0008-membership.md#acknowledgement) |
| `Notice`, `NoticeCode` | What `Members::add` and the other membership calls return: `code` and `text`, such as `NoticeCode::MemberAdded` or `NoticeCode::RoleSet` | [RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys) |
| `OpenOptions::pairing`, `OpenOptions::recover`, `Db::account`, `Account`, `PairKind`, `Paired`, `PairingAnswer`, `Removed`, `RecoveryCode` | Accounts in Rust (Carsten, 2026-11-12, NOC-273) | [RFC 0010, "Adding a device or an agent to an account"](0010-accounts.md#adding-a-device-or-an-agent-to-an-account) |
| `Transaction` | What a write's closure records on | [RFC 0002, "The atomic unit"](0002-change-and-log-format.md#the-atomic-unit) |
| `Transaction::create_with`, `Create` | `create` with options: `Create { same_person_as }`, the object of the same person in another space (Carsten, 2026-11-12, NOC-244) | [RFC 0005, "Identifying the person"](0005-schema-and-model-lock.md#identifying-the-person) |
| `Query`, `Page`, `Cursor`, `page.has_more` | A query, a page, its cursor, and whether more follow | [RFC 0006, "Ordered pages"](0006-local-view.md#ordered-pages) |
| `ObjectId`, `SpaceId`, `ChangeId`, `MemberId` | The ids, printed and parsed as lowercase hex | [RFC 0002, "Object ids"](0002-change-and-log-format.md#object-ids) |
| `Object` | A read object: `id()`, `space()`, `deleted()`, `erased()`, `get::<T>(name)` | [RFC 0005, "Types for the app"](0005-schema-and-model-lock.md#types-for-the-app) |
| `Written` | What `space.write` returns: `change` and `ids` | [RFC 0002, "The atomic unit"](0002-change-and-log-format.md#the-atomic-unit) |
| `Value`, `Value::add`, `Value::remove`, `Value::clear` | A field value, set operations, and the clear of a field (`Value::clear`: Carsten, 2026-11-12, NOC-277) | [RFC 0004, "The arguments of each operation"](0004-merge-rules-basic-types.md#the-arguments-of-each-operation) |
| `db.index` | The handle of a declared index, by collection and name | [RFC 0005, "Indexes"](0005-schema-and-model-lock.md#indexes) |
| `Error`, `ErrorCode`, `Error::fingerprint` | The error, its codes, and the fingerprint `UnknownAccount` carries | [RFC 0003, "Keys never reach logs, errors or telemetry"](0003-keys-and-cryptography.md#keys-never-reach-logs-errors-or-telemetry) |

## How this RFC is tested

`scripts/rfc-0009.test.mjs` and `scripts/rfc-0009-integrated.test.mjs`, run by `repo:test` in `pnpm gates`, check that this file exists, links to RFC 0001 to 0008 and GENESIS.md, that every relative link and its anchor resolve, links from other files included, that it has a section for every part NOC-64 names, that it reads as a fresh text whose choices each name why and the options not taken (`scripts/rfc-0009-fresh.test.mjs`), that every SDK name its TypeScript examples use is listed under [Public names](#public-names) and every row there links the RFC it builds on, that it shows the Rust SDK, and that the [RFC index](README.md) lists it. The design itself is tested where it is built: the Rust SDK's calls in `crates/nocentraldb`, and each call against the convergence harness and the view, natively and in the browser, as it is built.

## History

- 2026-11-10 · First published (RFC 0001 to RFC 0010 rewritten as fresh texts, NOC-165)
