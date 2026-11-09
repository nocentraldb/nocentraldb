# RFC 0009: The SDK API for apps

REQUEST FOR COMMENTS · NOVEMBER 2026 · Ticket NOC-64

**Changelog.**

- **2026-11-09** · NOC-64 · The owner decided S1 to S18 as recommended (Carsten, 2026-11-09), in chat with pm, on #78.

This RFC proposes the API an app or an agent uses to work with NoCentralDB: how it opens a database, writes and reads objects, follows them live, creates spaces and adds members, and learns how far its copy is synced and how safe it is. It proposes the API in TypeScript (`@nocentraldb/sdk`) and in Rust (`nocentraldb`), the two SDKs [GENESIS.md](../../factory/GENESIS.md) names.

The decided RFCs define what lies under these calls, and this RFC names the calls in between. [RFC 0005](0005-schema-and-model-lock.md) names the schema functions and `model.lock.json`. [RFC 0006](0006-local-view.md) names `db.durability` and the options of `nc.open` ([Amendment 2](0006-local-view.md#amendment-2-durability-of-a-browser-copy)). [RFC 0001](0001-nocentraldb.md#the-design-in-one-page) shows `db.find` with `take` and `after`. [RFC 0002](0002-change-and-log-format.md) defines the change a write becomes, [RFC 0004](0004-merge-rules-basic-types.md) how each field merges and what a read can show, [RFC 0003](0003-keys-and-cryptography.md) and [RFC 0008](0008-membership.md) the keys, roles and members, and [RFC 0007](0007-relay.md) the relay. This RFC changes none of them. Every name below links the RFC it builds on, and where an RFC is silent, this RFC says so.

**Status.** Decided. GENESIS.md reserves public API names for the owner ("Anything a user sees first"), and RFC 0004 (decision 7), RFC 0005 (D2) and RFC 0008 (M12) left the names of reads and roles to "the SDK's API review", which this RFC is. The owner accepted the recommendation of every decision in [Decisions for the owner](#decisions-for-the-owner), S1 to S18 (Carsten, 2026-11-09), and the design above describes those choices. Every name is collected in [Public names](#public-names). The points in [Conflicts](#conflicts-with-genesismd-and-the-other-rfcs) change other RFCs and documents, which their own amendments carry out. Nothing here is built.

**How to give feedback.** This RFC has no discussion of its own yet: start one in the category [RFC feedback](https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback), named after the RFC, or open a pull request against this file to change its text. [Giving feedback](README.md#giving-feedback) says how, and what happens to your point.

## What this RFC does not cover

- **Computed fields and views.** RFC 0005 reserves `computed`, `views`, `nc.reach` and `nc.view`; a later RFC designs them. This RFC lets a view be queried like an index, with `take` and `after`, as RFC 0001 shows, and designs nothing else about it.
- **Rich text and streams.** `nc.richText` and `nc.stream` are reserved (RFC 0005). Their write and read calls, such as an anchored edit or appending a chunk, belong to their RFCs.
- **Work orders and erasure.** How an erasure is started, claimed and signed off is the work-order RFC's. RFC 0003's sketch `db.workOrders.erase` stays a sketch.
- **Accounts.** How a second device or an agent joins a person's account, and recovery. This RFC names two entry points (decision [S15](#decisions-for-the-owner)); the accounts RFC designs what happens behind them.
- **Peers in island mode.** How two members find each other is a later RFC ([RFC 0007, "Island mode"](0007-relay.md#island-mode)), so the SDK has no peer API yet.
- **Bindings for UI frameworks**, such as a React hook. They can wrap `subscribe` later.
- **Code.** Nothing in `crates/` or `packages/` changes.

## Terms

| Term | Meaning in this RFC |
| --- | --- |
| Database, `db` | What `nc.open` returns: one member's copy of all its spaces on one device ([RFC 0006, decision 1](0006-local-view.md#decisions-for-the-owner)). |
| Space handle | What `db.spaces.get` and `db.spaces.create` return: the calls of one space. The examples name it `project`. |
| Transaction | The writes an app collects in one `space.write` call. It becomes exactly one change ([RFC 0002, "The atomic unit"](0002-change-and-log-format.md#the-atomic-unit)). |
| Query | A declared index, the values of its equality prefix, a page size and an optional cursor ([RFC 0006, "Declared indexes"](0006-local-view.md#declared-indexes)). |
| Page | At most N objects of a query in index order, and the cursor after the last one ([RFC 0006, "Ordered pages"](0006-local-view.md#ordered-pages)). |
| Report | A value the SDK keeps current and an app can read or follow, with `current()` and `subscribe(listener)`, in the shape [RFC 0006, decision 10](0006-local-view.md#amendment-2-durability-of-a-browser-copy) gives `db.durability`. |

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
- **Objects live in a space.** Reads and writes of single objects go through a space handle. Queries go through `db`, because an index spans every space of the member, unless the index is declared per space.
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
  persistentStorage: "on-open",       // RFC 0006, decision 11; or "on-request"
  warnings: true,                     // RFC 0006, decision 15
});

await db.close();
```

**What `nc.open` does,** in this order:

1. **Opens the member's database** on this device, and creates the member key on first open ([RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys)). In a browser it starts the worker, and if another tab owns the database, it reaches that tab instead ([RFC 0006, decision 4](0006-local-view.md#decisions-for-the-owner)).
2. **Checks the model against the lock.** The core folds only by the lock ([RFC 0005](0005-schema-and-model-lock.md), "The core and both SDKs read only the lock"). The TypeScript model gives the app its types and its handles, such as `model.tasks`. `nc.open` checks that every collection, field and index the model names is in the lock with the same type and options, and fails with `lockOutOfDate` otherwise, because a developer who changed the schema and did not run `ncdb model build` would write fields no lock knows (decision [S1](#decisions-for-the-owner)).
3. **Checks the lock on the device.** It runs the additive check between the lock the database was last opened with and this one, and refuses with `modelRefused`, naming each refused change, if the `model.id` differs or the change is not additive. It deletes and rewrites nothing ([RFC 0005, "Two checks: at build and on the device"](0005-schema-and-model-lock.md#two-checks-at-build-and-on-the-device), D11).
4. **Brings the view up to date.** It adds columns, tables and indexes for an additive change, folds parked operations that are now known, and starts a full rebuild where RFC 0006 asks for one ([RFC 0006, "Rebuilding the view"](0006-local-view.md#rebuilding-the-view)). `nc.open` resolves before a rebuild ends; the app reads the old view meanwhile, and `db.view` reports the rebuild's progress.
5. **Starts syncing** in the background when a relay is given ([Sync and status](#sync-and-status)), and asks the browser for persistent storage unless the app opened with `persistentStorage: "on-request"` ([RFC 0006, decision 11](0006-local-view.md#amendment-2-durability-of-a-browser-copy)).

**Spaces of another model.** A space records the `model.id` it was created with in its genesis ([RFC 0008, "What it names"](0008-membership.md#what-it-names)). `nc.open` does not fail because one space belongs to another model. That space's changes are not folded, nothing is written into it, and `db.spaces.get` on it throws `spaceOfAnotherModel`, an error that names both model ids, as NOC-57 builds it. `db.spaces.list()` shows the space with that state, so an app can tell its user which spaces it cannot open.

**No persistent storage** is not an error. A browser copy opens as `best-effort` or, where OPFS is missing, as `temporary` in memory, and `db.durability` reports it ([RFC 0006, decisions 10 and 12](0006-local-view.md#amendment-2-durability-of-a-browser-copy)). A full disk or quota fails a write with `storageFull`, never silently ([RFC 0006, "The browser"](0006-local-view.md#the-browser)).

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

**What a transaction is, and is not.**

- **Atomic in its checks, in one space.** The signature, the membership check and the decryption cover the change as a whole, so every member accepts all of it or none ([RFC 0002, D10](0002-change-and-log-format.md#the-atomic-unit)). A change never spans two spaces, so neither does a write. An app that writes into two spaces makes two writes, and each may be accepted or rejected on its own.
- **Not isolated.** The function records; it does not read. It gets no view of the transaction's own writes, and it must not await anything: the SDK collects the records when the function returns (decision [S3](#decisions-for-the-owner)). Read first, then write.
- **No guard against concurrent writes.** Another member may write the same field at the same time, offline. The fields merge by their types, and a write that loses stays identifiable ([RFC 0002, "A late change stays identifiable"](0002-change-and-log-format.md#a-late-change-stays-identifiable)). There is no "write only if unchanged", because no member can know what an offline member does (RFC 0001, [What it is not](0001-nocentraldb.md#what-it-is-not)).
- **An old client may show part of it.** A transaction that writes a field an old client does not know shows on that client without that field, until it upgrades ([RFC 0002, "The atomic unit"](0002-change-and-log-format.md#the-atomic-unit)).
- **The net effect per field.** A transaction that writes one field twice writes its last value; an element added and then removed in one transaction is written as RFC 0004 says ([RFC 0004, "The arguments of each operation"](0004-merge-rules-basic-types.md#the-arguments-of-each-operation)). An operation cannot name another operation of its own change, so the SDK encodes the net effect.

### Field operations per field type

`values` maps field names to what to write. The field's type decides what a value means ([RFC 0005, "Field types"](0005-schema-and-model-lock.md#field-types); [RFC 0004](0004-merge-rules-basic-types.md)):

| Field | Write | Operation |
| --- | --- | --- |
| `nc.string`, `nc.int`, `nc.float`, `nc.boolean`, `nc.oneOf` | The value, such as `{ priority: 3 }`. `null` clears the field. | set (1). `replaces` names the winner and every conflict the copy holds ([RFC 0004, "Basic values"](0004-merge-rules-basic-types.md#basic-values)). |
| `nc.link` | The target's object id, or `null`. | set (1), as a basic value. |
| `nc.set`, `nc.links` | `nc.add(...elements)` or `nc.remove(...elements)`. | add (2) per element, or remove (3) per element with `observed`, the tags of its additions the copy holds ([RFC 0004, "Sets"](0004-merge-rules-basic-types.md#sets)). |
| A plugin's type | What the plugin's write function returns. | The plugin's operations, 64 and up. |

- **Every write resolves the conflicts the copy holds.** A set replaces the winner and every conflict on this device, as RFC 0004 defines it. An app that shows conflicts to a person before writing gets what RFC 0004 intends: the person decides with the conflict in front of them. An app that writes without looking resolves them too; [Read and query](#read-and-query) says how to find them first.
- **A deliberate re-add.** `nc.add(e)` names the lost removals of `e` the copy holds in `replaces`, so they stop counting as lost ([RFC 0004, "Sets"](0004-merge-rules-basic-types.md#sets)).
- **A set is never written whole.** There is no "set the tags to exactly these", because it would remove what a concurrent member added without its author having seen it (decision [S4](#decisions-for-the-owner)).
- **`undefined` writes nothing.** A key with the value `undefined` is ignored, so an app can pass a partial object. Every field may be unset ([RFC 0005, "No required fields and no defaults"](0005-schema-and-model-lock.md#no-required-fields-and-no-defaults)).
- **Strings are NFC-normalised** before they are written ([RFC 0005, "Field types"](0005-schema-and-model-lock.md#field-types)).

**What the SDK refuses,** before anything is written, with an error and no change:

- a value that does not fit the field's kind, such as a text in an `nc.int` field, an option the lock does not list, or a value nested deeper than RFC 0004 allows: `invalidValue` ([RFC 0005, "Data from a newer model"](0005-schema-and-model-lock.md#data-from-a-newer-model), point 5);
- a deprecated field or collection: `deprecated` ([RFC 0005, "Modifiers"](0005-schema-and-model-lock.md#modifiers));
- an update, delete or restore of an object the copy does not hold: `notFound`. The format allows such a write ([RFC 0004, "Creating and deleting an object"](0004-merge-rules-basic-types.md#creating-and-deleting-an-object)), but an app that writes to an id it cannot read has most likely a wrong id. This is a rule of the SDK, not of the format;
- a write into a space where this member may not write sealed changes, because it is a reader or no longer a member: `notAllowed` or `notMember` ([RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights));
- a write into a space of another model: `spaceOfAnotherModel` ([RFC 0005, D13](0005-schema-and-model-lock.md#decisions-for-the-owner)).

### Personal fields

A personal field is written like any other field. The SDK seals it under the key of the person it is about, which it finds from the schema's `person` ([RFC 0005, "Personal fields"](0005-schema-and-model-lock.md#personal-fields)):

- `person: "self"`: the object itself is the person.
- `person: "<link field>"`: the target of that link, as the copy holds it after this transaction's own writes. A transaction that sets `assignee` and `notesAboutAssignee` together seals the notes under the new assignee's key.

RFC 0005 is silent on a personal field whose `person` link is unset. This RFC proposes that the SDK refuses the write with `noPerson`: sealing it under no person would make it unerasable, and choosing a person would guess.

**The same person in another space.** RFC 0005's D14 gives a person a person id that the core carries over when the app records the same person in another space, and leaves to this RFC how the app says "the same person". `tx.create` takes the option `samePersonAs`, an object of the same person read from another space:

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
- a choice as the union of its options, and `nc.Unknown`, `{ unknown: rawValue }`, for an option the lock does not list yet (decision [S5](#decisions-for-the-owner));
- a set as an array, in the order of its elements' canonical bytes; a link as the target's object id; many links as an array of ids.

An object id is unique in its space, not across spaces: a hostile member can create an id that exists in another space ([RFC 0002, "Object ids"](0002-change-and-log-format.md#object-ids)). So a single object is read through its space, and an object read from a query carries its space.

**What a read object carries beyond its fields.** Names that start with `$` are reserved for the core and never name a field ([RFC 0005, "Names"](0005-schema-and-model-lock.md#names)), so the SDK puts what it knows about the object there:

| Property | Meaning |
| --- | --- |
| `$id`, `$space` | The object id and the space id. |
| `$deleted` | `true` when the object is deleted. `space.get` returns deleted objects; queries do not (decision [S9](#decisions-for-the-owner)). |
| `$conflicts` | The names of the fields that have conflicts, lost removals, or a link that is not **ok**. Empty when there are none. |
| `$raw` | The names of the fields whose value does not fit the field's kind, such as a text in an `nc.int` field written by a buggy member. Such a field reads `undefined`; `space.inspect` gives the raw value ([RFC 0005, "Data from a newer model"](0005-schema-and-model-lock.md#data-from-a-newer-model), point 5). |

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
- **The object:** its collection and whether it is deleted, each a basic value with conflicts ([RFC 0004, "Two creates of one object id"](0004-merge-rules-basic-types.md#two-creates-of-one-object-id)).
- **A field outside the kind:** `{ raw: value }`.

`project.outcome(change, collection, id, field)` answers RFC 0004's `outcome(change, field)`: `"won"`, `"lost"` or `"replaced"` for a basic value, `"applied"`, `"lost"` or `"replaced"` for a set. These names are RFC 0004's placeholders; this RFC proposes them as the public names (decision [S6](#decisions-for-the-owner)).

### Queries and indexes

A query names a declared index, fixes its equality prefix and asks for a page ([RFC 0006, "Declared indexes"](0006-local-view.md#declared-indexes)). The model gives every declared index a handle:

```ts
const query = model.tasks.indexes.byStatusPriority.equal("open").take(50);
const page = await db.find(query);       // { items: Task[], cursor, hasMore }
const more = await db.find(query.after(page.cursor));
const firstFive = await db.find(model.tasks.indexes.byPriorityDesc.take(5));
```

- `.equal(...values)` fixes the index's leading fields, as many as its `equal` declares, no more and no fewer; TypeScript checks the count and the types ([RFC 0005, "Indexes"](0005-schema-and-model-lock.md#indexes)).
- `.take(n)` sets the page size, 50 by default, up to the maximum RFC 0006 leaves to the implementation.
- `.after(cursor)` starts after a page's cursor. A cursor from another model, or another index, fails with `cursorOfAnotherModel`, and the app starts from the first page ([RFC 0006, "Ordered pages"](0006-local-view.md#ordered-pages)).
- `db.find(query)` returns `items` in index order, the `cursor` after the last item, and `hasMore`. Every item carries `$space`, because an index spans every space of the member ([RFC 0006, decision 1](0006-local-view.md#decisions-for-the-owner)).
- A view, once the computed-fields RFC designs it, is queried the same way: `model.tasks.views.ready.take(50)`, as RFC 0001 shows.

There is nothing else: no `where` on a field outside the index, no sort of the app's choosing, no raw SQL. An app that needs another filter declares another index, or a computed field and a view ([GENESIS.md, principle 6](../../factory/GENESIS.md#principles-in-priority-order)).

**A query in one space.** An index ends with the space id and the object id ([RFC 0006, "Declared indexes"](0006-local-view.md#declared-indexes)), so it cannot fix the space: "the open tasks of Project A" is no query RFC 0006 allows. RFC 0005 and RFC 0006 are silent on it, and an app needs it on its first screen. This RFC proposes an index option, `scope: "space"`, that puts the space id first, and `space.find(query)`, which fixes it (decision [S8](#decisions-for-the-owner)):

```ts
// in the schema: indexes: { openByPriority: nc.index(["status", "priority"], { equal: 1, scope: "space" }) }
const mine = await project.find(model.tasks.indexes.openByPriority.equal("open").take(50));
```

This needs an amendment to RFC 0005 (the option and its key in the lock) and to RFC 0006 (the index's columns); see [Conflicts](#conflicts-with-genesismd-and-the-other-rfcs).

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
- **`db.changes.subscribe(listener)`** is the change event: after each fold, the space, collection and ids it touched, and whether the change was this member's own write. It carries ids, never values, as RFC 0006's notification does, so an adapter or an agent's wake-up logic reads the objects itself.
- Every `subscribe` returns a function that stops it, like `db.durability.subscribe` ([RFC 0006, decision 10](0006-local-view.md#amendment-2-durability-of-a-browser-copy)).
- **Un-folding is a change too.** When a late control change rejects a change that was folded, the fields it touched are folded again ([RFC 0006, "Control changes and changes that wait for them"](0006-local-view.md#control-changes-and-changes-that-wait-for-them)), and the listeners of those objects are called like after any fold. The app sees a value go back; `space.status` says why.

## Spaces and members

### Spaces

```ts
const project = await db.spaces.create();           // writes the genesis, offline if need be
const same = await db.spaces.get(project.id);
const all = await db.spaces.list();                 // [{ id, role, model: "ours" | "another", ... }]
await project.leave();                              // returns a notice
```

- **`db.spaces.create()`** writes the space's genesis and the creator's own key envelope, on the device, offline if need be ([RFC 0008, "The first change of a space"](0008-membership.md#the-first-change-of-a-space)). The creator is the space's first admin. The space id is derived from the creator and a nonce, so the app does not choose it. A relay that does not let this member create spaces answers `not_allowed` when the genesis is pushed ([RFC 0007, "The errors"](0007-relay.md#the-errors)); the space keeps working locally, and `space.status` reports the refusal.
- **A space has no name in the SDK.** The genesis holds no name, because a name is content the relay must not read ([RFC 0008, "What it names"](0008-membership.md#what-it-names)). RFC 0003's sketch `db.spaces.create({ name: "Project A" })` does not say where the name goes. This RFC proposes that the app keeps names in its own model, for example a `projects` collection with one object in each project's space (decision [S11](#decisions-for-the-owner)).
- **`space.leave()`** writes a removal of the member itself ([RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights)). The SDK refuses with `lastAdmin` when this member is the space's last admin, as RFC 0008 requires; it cannot prevent two admins from leaving at the same time ([RFC 0008, decision M13](0008-membership.md#decisions-for-the-owner)).
- **Deleting a space** that synced is not defined by any RFC yet ([RFC 0008, "What this RFC does not cover"](0008-membership.md#what-this-rfc-does-not-cover)), so the SDK offers no call for it.

### Invitations and members

```ts
// On the joining member, a device or an agent:
const invitation = await db.invitation();   // public: member id, public keys, certificate, fingerprint
send(invitation.toString());                 // the app carries it to an admin

// On an admin of the space:
const added = await project.members.add(nc.invitation(text), { role: "writer" });
added.notice;  // "The new member reads the whole history of this space."

await project.members.setRole(memberId, "reader");
const removal = await project.members.remove(memberId);
removal.notice; // "The removed member keeps its copy of everything it received before now."
const members = await project.members.list(); // [{ id, account, kind, role, self }]
```

- **An invitation is the joining member's public identity.** `db.invitation()` returns its member id, its two public keys, its member certificate and its account's fingerprint. Nothing in it is secret, and no key bytes reach the app ([RFC 0003, decision 12](0003-keys-and-cryptography.md#decisions-for-the-owner)). The app carries it to an admin by any channel: a link, a QR code, a message. RFC 0008 leaves this step out ("Invitations and public API names"), and RFC 0003 says only that the SDK takes the identity "from an invitation the new member accepted" (decision [S12](#decisions-for-the-owner)).
- **`members.add(invitation, { role })`** checks the certificate, writes the membership entry and the new member's key envelope, and returns a notice ([RFC 0003, "Sharing a space key with a new member"](0003-keys-and-cryptography.md#sharing-a-space-key-with-a-new-member)). When the account is new to this device, it fails with `unknownAccount` and the account's fingerprint, until the app passes `{ role, fingerprint }` after a person compared it, the way messaging apps compare safety numbers. `role` may be left out for an agent, which then joins as `"writer"` ([RFC 0008, decision M14](0008-membership.md#decisions-for-the-owner)); for a device it is required. `members.add` also takes a list of invitations, which writes one entry and one envelope per member, as the SDK's expansion of a group ([RFC 0008, "Groups"](0008-membership.md#groups); decision [S14](#decisions-for-the-owner)).
- **`members.setRole(memberId, role)`** raises or lowers a role; **`members.remove(memberId)`** removes a member and rotates the space key, as RFC 0003 requires. Both write a cutoff ([RFC 0008, "The cutoff"](0008-membership.md#the-cutoff)). Re-granting a right a member lost rotates first ([RFC 0008, decision M5](0008-membership.md#decisions-for-the-owner)). Only an admin may call them; others get `notAllowed`.
- **Roles** are `"reader"`, `"writer"` and `"admin"`, RFC 0008's working names, proposed as the public names (decision [S13](#decisions-for-the-owner)).
- **`members.list()`** returns each member's id, its account fingerprint, its `kind` (`"device"`, `"agent"` or `"subAgent"`, from its certificate), its role, and whether it is this member. Every member is visible, an agent included ([GENESIS.md, "What it must never do"](../../factory/GENESIS.md#what-it-must-never-do)).
- **Notices.** Every call that changes who can read or write returns a `notice`: `{ code, text }`, the text in plain English for the developer, saying what the change does not do ([RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys)). The SDK ships no text for end users ([RFC 0006, decision 14](0006-local-view.md#amendment-2-durability-of-a-browser-copy)).

**What membership calls do not do,** stated as their notices state it: a removed member keeps its copy; a revocation does not take back what a member read; a member that has not seen a removal treats the removed member as it was until the removal reaches it; and until a merge rotation lands, members may write under a key generation a removed member holds ([RFC 0008, "Two removals that rotate the key at once"](0008-membership.md#two-removals-that-rotate-the-key-at-once)).

**Outcomes that change later.** A grant concurrent with a removal of the same member loses, and the SDK shows it as a lost grant ([RFC 0008, "Concurrent add and remove of one member"](0008-membership.md#concurrent-add-and-remove-of-one-member)). `space.status` lists lost grants, and `members.list()` shows the role that resulted, not the one asked for.

### Sub-agents and ad-hoc spaces

```ts
// The parent agent:
const { space: adhoc, notice } = await db.spaces.createAdHoc({ copy: [taskA, taskB], personal: false });
await adhoc.members.add(subAgentInvitation, { role: "writer" });
const result = await adhoc.get(model.tasks, copyOfTaskA);
await project.update(model.tasks, taskA.$id, { status: result.status });
```

- **`db.spaces.createAdHoc({ copy, personal, local })`** creates a space with a fresh key and copies the given objects into it as new objects ([RFC 0003, "Sub-agents get an ad-hoc space"](0003-keys-and-cryptography.md#sub-agents-get-an-ad-hoc-space)). Personal fields are not copied unless `personal: true` ([RFC 0003, decision 11](0003-keys-and-cryptography.md#decisions-for-the-owner)), and then the person key is shared into the space. `local: true` keeps the space off the relay, for a sub-agent on the same machine. The notice says that the space limits what the sub-agent can decrypt, not what a process running as the same user can read.
- **A sub-agent** opens its own database with `nc.open({ model, lock, subAgent: true })`; its invitation carries no certificate, and the parent's `members.add` signs one with the parent's member key ([RFC 0008, decision M25](0008-membership.md#decisions-for-the-owner)).
- **The source of a copy.** RFC 0003 says the copies "keep a reference to their source", and no RFC says where. This RFC proposes that the parent's SDK keeps the mapping locally, which is all the parent needs to write results back; the sub-agent never learns the source's id.
- **Deleting the ad-hoc space** is a local deletion while it stayed local (`adhoc.discard()`); a synced one waits for the RFC that deletes spaces.

### Accounts and devices

RFC 0003 defines the account key and pairing in steps, and leaves the flow to a person confirming on a device. This RFC only names the entry points, so the rest of the API can refer to them (decision [S15](#decisions-for-the-owner)): `db.account.pair({ kind })` on a device that holds the account key starts pairing a new device or agent, and `nc.open({ model, lock, pairing })` on the new one completes it. `db.account.removeMember(memberId)` removes a lost device or an agent from every space the account administers, as RFC 0003 asks ("the SDK offers this as one action on the account"). The accounts RFC designs what happens behind these calls ([RFC 0003, "The account key"](0003-keys-and-cryptography.md#the-account-key)).

## Sync and status

```ts
const sync = db.sync.current();
// { connection: "connected", relay: "wss://relay.example.com", unconfirmed: 3, refused: 0, epochChanged: false }

const status = project.status.current();
// { role, initialSyncComplete, behind, gaps, parked, rejected, refused, lostGrants,
//   equivocations, exposed, waitingForKey, shredsWaiting }

const stop = db.sync.subscribe((sync) => showConnection(sync));
await db.sync.pause();
await db.sync.resume();
```

**The relay connection.** `nc.open({ relay })` names one relay for the database (decision [S16](#decisions-for-the-owner)). The SDK opens one connection, authenticates with the member key, pulls, pushes and subscribes for every space, and reconnects with backoff when the connection drops ([RFC 0007, "Island mode"](0007-relay.md#island-mode)). The app never sees the protocol. `db.sync.pause()` and `resume()` stop and restart it, for an app that wants to stay offline.

**`db.sync`**, a report:

| Value | Meaning |
| --- | --- |
| `connection` | `"connected"`, `"connecting"`, `"offline"` (no relay reachable, the SDK retries), `"paused"` or `"none"` (opened without a relay). |
| `unconfirmed` | This member's changes the relay has not confirmed, as `db.durability` counts them ([RFC 0006, decision 10](0006-local-view.md#amendment-2-durability-of-a-browser-copy)). |
| `refused` | Changes of this member the relay refused ([RFC 0002, D12](0002-change-and-log-format.md#a-change-the-relay-refuses-after-the-member-folded-it)). |
| `epochChanged` | The relay was restored and the SDK is pushing again what it lost ([RFC 0007, "Restores and the relay epoch"](0007-relay.md#restores-and-the-relay-epoch)). |
| `error` | The last error that stops syncing, such as `unsupported_version`, by its RFC 0007 code; never content. |

**`space.status`**, a report per space, with everything a space can hold that an app does not see in a read:

| Value | Meaning |
| --- | --- |
| `role` | This member's role, or `"removed"`. A removed member keeps reading its copy. |
| `initialSyncComplete`, `behind` | Whether the first pull reached the head, and how many relay positions the member has heard of but not folded ([RFC 0006, "Reads never touch the network"](0006-local-view.md#reads-never-touch-the-network)). |
| `gaps` | Dependencies the member does not hold ([RFC 0002, "Dependencies"](0002-change-and-log-format.md#dependencies)). |
| `parked` | Per reason, the parked changes and operations, and the highest model version they need; "another model" apart ([RFC 0005, "Data from a newer model"](0005-schema-and-model-lock.md#data-from-a-newer-model)). |
| `rejected` | Per reason, the rejected changes ([RFC 0002, "Rejected changes"](0002-change-and-log-format.md#rejected-changes)). |
| `refused` | This member's changes in this space the relay refused, by change id and code. They stay folded on this member and do not reach members that sync only through the relay; the app may write a change that reverts one ([RFC 0002, D12](0002-change-and-log-format.md#a-change-the-relay-refuses-after-the-member-folded-it)). |
| `lostGrants`, `equivocations` | RFC 0008's lost grants, and members that wrote two changes under one sequence ([RFC 0008, "The cutoff"](0008-membership.md#the-cutoff)). |
| `exposed`, `waitingForKey` | Whether members write under a generation a removed member may hold, and whether this member waits for an envelope ([RFC 0008, "Two removals that rotate the key at once"](0008-membership.md#two-removals-that-rotate-the-key-at-once)). |
| `shredsWaiting` | Shreds that are not stable yet, with their age and, on an admin device after 7 days, the admin devices that block them ([RFC 0008, "Effects that wait until a change is stable"](0008-membership.md#effects-that-wait-until-a-change-is-stable), M21). |

**Durability** is RFC 0006's `db.durability`, unchanged: `current()`, `subscribe(listener)` and `requestPersistent()`, with `storage`, `unconfirmed`, `oldestUnconfirmedAt` and `atRisk` ([RFC 0006, decision 10](0006-local-view.md#amendment-2-durability-of-a-browser-copy)). It is on every target; natively `storage` is `"persistent"`.

**`db.view`**, a report of the view itself: whether a rebuild runs and how far it is ([RFC 0006, "What a rebuild costs"](0006-local-view.md#what-a-rebuild-costs)).

**Confirmation of one write.** `write` resolves on the device. An app that must know when the relay holds a change reads `db.sync.isConfirmed(result.change)` or waits for it with `db.sync.confirmed(result.change)`, a promise that resolves when the relay records a position for it. It may never resolve: the relay can refuse the change, or never be reached (decision [S3](#decisions-for-the-owner)).

## Errors

The SDK throws one error class, `nc.Error`, with a string `code` and the ids it concerns (decision [S17](#decisions-for-the-owner)). An error never carries content, a key or a personal field: "Decryption failed for change C in space S, generation 4" is the model ([RFC 0003, "Keys never reach logs, errors or telemetry"](0003-keys-and-cryptography.md#keys-never-reach-logs-errors-or-telemetry)).

| Code | Thrown by | When |
| --- | --- | --- |
| `lockOutOfDate` | `nc.open` | The model names a collection, field or index the lock does not have, or with another type. Run `ncdb model build`. |
| `modelRefused` | `nc.open` | The lock has another `model.id` than the stored one, or is not an additive successor; lists each refused change ([RFC 0005, D11](0005-schema-and-model-lock.md#decisions-for-the-owner)). |
| `spaceOfAnotherModel` | `db.spaces.get`, writes | The space's genesis names another `model.id`; names both ids. |
| `storageFull` | writes, `nc.open` | The disk or the browser's quota is full. |
| `closed` | every call | The database was closed. |
| `notFound` | `space.update`, `delete`, `restore`, `db.spaces.get` | The copy holds no such object or space. |
| `invalidValue`, `deprecated`, `noPerson` | writes | See [Field operations per field type](#field-operations-per-field-type) and [Personal fields](#personal-fields). |
| `notAllowed`, `notMember`, `lastAdmin` | writes, membership calls | The member's role does not allow it, it is no longer a member, or it is the last admin. |
| `unknownAccount` | `members.add` | The invitation's account is new to this device; carries its fingerprint. |
| `cursorOfAnotherModel` | `db.find` | The cursor was made under another model or index. |

What syncing finds is never thrown, because no call of the app caused it: a relay refusal, a rejected or parked change, a lost grant or a gap arrives in a report.

## The Rust SDK

The Rust SDK, crate `nocentraldb`, has the same calls in snake case, async, and the same behaviour on every target ([GENESIS.md, principle 7](../../factory/GENESIS.md#principles-in-priority-order)). It reads the lock at build time or at run time; RFC 0005 says typed code can be generated from the lock later, so v1 reads fields by name (decision [S18](#decisions-for-the-owner)):

```rust
use nocentraldb::{Db, OpenOptions, Value};

let db = Db::open(OpenOptions::new(include_str!("../model.lock.json"))
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
- `space.write(|tx| ...)` takes a closure that records; it is synchronous for the same reason as in TypeScript.
- `subscribe` returns a `Stream`; dropping it stops it. Reports are `db.sync().current()` and `db.sync().subscribe()`.
- Errors are `nocentraldb::Error` with an `ErrorCode` enum whose variants are the codes above in Rust case, such as `ErrorCode::LockOutOfDate`. `Debug` of every type that holds content or a cursor prints `[redacted]` ([RFC 0003, "Keys never reach logs, errors or telemetry"](0003-keys-and-cryptography.md#keys-never-reach-logs-errors-or-telemetry)).

## Public names

Every public name this RFC proposes or uses, for the owner to approve in one place, each with the RFC it builds on. "Decided" means another RFC decided the name already; this RFC only uses it.

**Open and close** (`@nocentraldb/sdk`):

| Name | What it is | Builds on |
| --- | --- | --- |
| `nc.open` | Opens this member's database; options `model`, `lock`, `relay`, `persistentStorage` (`"on-open"`, the default this RFC names, or `"on-request"`), `warnings`, `subAgent`, `pairing` | Decided with `model`, `persistentStorage`, `warnings`: [RFC 0006, decisions 11 and 15](0006-local-view.md#amendment-2-durability-of-a-browser-copy); `lock`: [RFC 0005](0005-schema-and-model-lock.md#two-checks-at-build-and-on-the-device) |
| `db.close` | Closes the database | [RFC 0006, "One writer, many readers"](0006-local-view.md#one-writer-many-readers) |
| `nc.Error` | The one error class, with `code` | [RFC 0003, "Keys never reach logs, errors or telemetry"](0003-keys-and-cryptography.md#keys-never-reach-logs-errors-or-telemetry) |

**Write:**

| Name | What it is | Builds on |
| --- | --- | --- |
| `space.write` | Records a transaction; one change | [RFC 0002, "The atomic unit"](0002-change-and-log-format.md#the-atomic-unit) |
| `tx.create` | Creates an object; option `samePersonAs` | [RFC 0004, "Creating and deleting an object"](0004-merge-rules-basic-types.md#creating-and-deleting-an-object); [RFC 0005, D14](0005-schema-and-model-lock.md#decisions-for-the-owner) |
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
| `space.inspect` | Reads one object with conflicts, lost removals and link statuses | [RFC 0004, decision 7](0004-merge-rules-basic-types.md#decisions-for-the-owner) |
| `space.outcome` | The outcome of one change on one field | [RFC 0004, "Late changes stay identifiable"](0004-merge-rules-basic-types.md#late-changes-stay-identifiable) |
| `value`, `conflicts`, `elements`, `lostRemovals`, `status`, `raw`, `author`, `stamp` | The parts of `inspect` | [RFC 0004, decision 7](0004-merge-rules-basic-types.md#decisions-for-the-owner) |
| `"ok"`, `"deleted"`, `"missing"`, `"wrongCollection"` | Link statuses | [RFC 0004, "Resolution"](0004-merge-rules-basic-types.md#resolution) |
| `"won"`, `"lost"`, `"replaced"`, `"applied"` | Outcomes | [RFC 0004, "Late changes stay identifiable"](0004-merge-rules-basic-types.md#late-changes-stay-identifiable) |
| `nc.Unknown` | A choice option the lock does not list: `{ unknown }` | [RFC 0004, "Value rules per kind"](0004-merge-rules-basic-types.md#value-rules-per-kind) |
| `$id`, `$space`, `$deleted`, `$conflicts`, `$raw` | What a read object carries beyond its fields | [RFC 0005, "Names"](0005-schema-and-model-lock.md#names) |
| `db.find` | Runs a query across the member's spaces | Decided name: [RFC 0001, "The design in one page"](0001-nocentraldb.md#the-design-in-one-page) |
| `space.find` | Runs a query of an index scoped to one space | [RFC 0006, "Declared indexes"](0006-local-view.md#declared-indexes) |
| `model.<collection>.indexes.<index>` | The handle of a declared index | [RFC 0005, "Indexes"](0005-schema-and-model-lock.md#indexes) |
| `.equal(` | Fixes the index's equality prefix | [RFC 0006, "Declared indexes"](0006-local-view.md#declared-indexes) |
| `.take(` | The page size | Decided name: [RFC 0001, "The design in one page"](0001-nocentraldb.md#the-design-in-one-page) |
| `.after(` | Starts after a cursor | Decided name: [RFC 0001, "The design in one page"](0001-nocentraldb.md#the-design-in-one-page) |
| `items`, `cursor`, `hasMore` | The parts of a page | [RFC 0006, "Ordered pages"](0006-local-view.md#ordered-pages) |
| `nc.index` | Declares an index; this RFC adds the option `scope` | Decided name: [RFC 0005, "Public names"](0005-schema-and-model-lock.md#public-names) |
| `scope: "space"` | Index option: the index's first column is the space id | [RFC 0005, "Indexes"](0005-schema-and-model-lock.md#indexes), amended |

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
| `db.spaces.get`, `db.spaces.list` | A space handle; every space of the member | [RFC 0006, decision 1](0006-local-view.md#decisions-for-the-owner) |
| `db.spaces.createAdHoc` | An ad-hoc space for a sub-agent; options `copy`, `personal`, `local` | [RFC 0003, "Sub-agents get an ad-hoc space"](0003-keys-and-cryptography.md#sub-agents-get-an-ad-hoc-space) |
| `space.id` | The space id | [RFC 0008, "What it names"](0008-membership.md#what-it-names) |
| `space.leave` | Removes this member from the space | [RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights) |
| `space.discard` | Deletes a local ad-hoc space | [RFC 0003, "Sub-agents get an ad-hoc space"](0003-keys-and-cryptography.md#sub-agents-get-an-ad-hoc-space) |
| `db.invitation`, `nc.invitation` | This member's invitation; parses one from text | [RFC 0003, decision 12](0003-keys-and-cryptography.md#decisions-for-the-owner) |
| `space.members.add` | Adds members by invitation; options `role`, `fingerprint` | Sketched in [RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys) |
| `space.members.remove` | Removes a member and rotates the key | Sketched in [RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys) |
| `space.members.setRole` | Raises or lowers a role | [RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights) |
| `space.members.list` | The members, with `id`, `account`, `kind`, `role`, `self` | [RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights) |
| `"reader"`, `"writer"`, `"admin"` | Roles | [RFC 0008, decision M12](0008-membership.md#decisions-for-the-owner) |
| `"device"`, `"agent"`, `"subAgent"` | Kinds of member | [RFC 0003, "The account key"](0003-keys-and-cryptography.md#the-account-key) |
| `notice` | What a membership call does not do: `{ code, text }` | [RFC 0003, decision 12](0003-keys-and-cryptography.md#decisions-for-the-owner) |
| `db.account.pair`, `db.account.removeMember` | Entry points of the accounts RFC | [RFC 0003, "The account key"](0003-keys-and-cryptography.md#the-account-key) |

**Sync and status:**

| Name | What it is | Builds on |
| --- | --- | --- |
| `db.sync.current`, `db.sync.subscribe` | The relay connection, as a report | [RFC 0007, "Island mode"](0007-relay.md#island-mode) |
| `db.sync.pause`, `db.sync.resume` | Stop and restart syncing | [RFC 0007, "Island mode"](0007-relay.md#island-mode) |
| `db.sync.isConfirmed`, `db.sync.confirmed` | Whether, and when, the relay holds a change | [RFC 0007, "Push: append signed, encrypted changes"](0007-relay.md#push-append-signed-encrypted-changes) |
| `space.status.current`, `space.status.subscribe` | What a space holds and a read does not show | [RFC 0002, "Rejected changes"](0002-change-and-log-format.md#rejected-changes); [RFC 0005, "Data from a newer model"](0005-schema-and-model-lock.md#data-from-a-newer-model) |
| `db.durability.current`, `db.durability.subscribe`, `db.durability.requestPersistent` | Durability of the copy | Decided: [RFC 0006, decision 10](0006-local-view.md#amendment-2-durability-of-a-browser-copy) |
| `db.view.current`, `db.view.subscribe` | Progress of a rebuild | [RFC 0006, "What a rebuild costs"](0006-local-view.md#what-a-rebuild-costs) |

**Errors:**

| Name | What it is | Builds on |
| --- | --- | --- |
| `lockOutOfDate`, `modelRefused`, `spaceOfAnotherModel` | Model errors | [RFC 0005, "Two checks: at build and on the device"](0005-schema-and-model-lock.md#two-checks-at-build-and-on-the-device) |
| `storageFull`, `closed`, `notFound`, `cursorOfAnotherModel` | Errors of the view | [RFC 0006, "Ordered pages"](0006-local-view.md#ordered-pages) |
| `invalidValue`, `deprecated`, `noPerson` | Errors of a write | [RFC 0005, "Data from a newer model"](0005-schema-and-model-lock.md#data-from-a-newer-model) |
| `notAllowed`, `notMember`, `lastAdmin`, `unknownAccount` | Errors of rights and membership | [RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights) |

**Rust** (`nocentraldb`): every name above in snake case, and:

| Name | What it is | Builds on |
| --- | --- | --- |
| `Db::open`, `OpenOptions` | Opens the database from the lock's text | [RFC 0005, "Two checks: at build and on the device"](0005-schema-and-model-lock.md#two-checks-at-build-and-on-the-device) |
| `Value`, `Value::add`, `Value::remove` | A field value, and set operations | [RFC 0004, "The arguments of each operation"](0004-merge-rules-basic-types.md#the-arguments-of-each-operation) |
| `db.index` | The handle of a declared index, by collection and name | [RFC 0005, "Indexes"](0005-schema-and-model-lock.md#indexes) |
| `Error`, `ErrorCode` | The error and its codes | [RFC 0003, "Keys never reach logs, errors or telemetry"](0003-keys-and-cryptography.md#keys-never-reach-logs-errors-or-telemetry) |

## Rejected options

| Option | Why not |
| --- | --- |
| An ORM in which an object is a class instance that saves itself | Hides when a change is written and how many changes a save makes; one write must be one visible change, with its own checks. |
| A `where` clause on any field, as use case 7 shows | Needs a scan, which [GENESIS.md, principle 6](../../factory/GENESIS.md#principles-in-priority-order) rules out. Queries run only through declared indexes. |
| Raw SQL against the view | Rejected by [RFC 0006](0006-local-view.md#rejected-options): it could scan, and read the merge state and the log. |
| A write that resolves only when the relay accepts it | Puts the network on the write path, so island mode would stall every write; RFC 0006 folds the member's own write first. A promise per write is offered instead (`db.sync.confirmed`). |
| A compare-and-set write ("only if the value is still X") | No member can know what an offline member wrote; it would fail or succeed differently per member ([RFC 0001, "What it is not"](0001-nocentraldb.md#what-it-is-not)). |
| Transactions across spaces | A change never spans two spaces ([RFC 0002, "Operations: object, field and value"](0002-change-and-log-format.md#operations-object-field-and-value)). |
| Every field wrapped as `{ value, conflicts }` in every read | Most reads show values; conflicts are rare. The `$conflicts` flag tells the app where to look, and `inspect` gives the detail. |
| Writing a set as a whole array, with the SDK computing the difference | Removes what a concurrent member added that the writer had not seen, as a removal of elements it observed; the intent "exactly these" cannot converge under add wins. |
| Throwing on relay refusals, rejected or parked changes | They arrive while no app call runs, and they are states of the copy, not failures of a call. Reports keep them readable at any time. |
| One space handle per object type, such as `db.tasks.create` | Mixes the model into the database's namespace, where a collection named `spaces` or `sync` would collide. |
| The SDK reads the TypeScript model alone and builds the lock in memory | The lock is the contract every runtime reads ([RFC 0005, "Rejected options"](0005-schema-and-model-lock.md#rejected-options)); building it on the device would skip the build's history and its check. |

## Decisions for the owner

GENESIS.md reserves public API names for the owner. Each decision names the options and a recommendation, and ends with the owner's decision. The owner accepted every recommendation (Carsten, 2026-11-09), and the design above describes them.

**S1. Where the SDK gets the model.**

- A. `nc.open({ model, lock })`: the lock gives the rules, the TypeScript model the types and handles, and `nc.open` checks that they agree. *(recommended)*
- B. `nc.open({ lock })`, typed with a generic, `nc.open<typeof model>({ lock })`, and handles from the lock by name.
- C. `ncdb model build` also writes a module that exports both, and the app opens with that module.

Recommendation: A. RFC 0005 says the core reads only the lock, and RFC 0006 already decided `nc.open({ model, ... })`, so A keeps both and only adds `lock`. The check catches the most common mistake, a schema edited without a build. B loses the handles that make queries typed. C adds a generated file to every app's repository.

Decided (Carsten, 2026-11-09): the recommendation, (A).

**S2. Where single objects are read and written.**

- A. Through a space handle: `project.get`, `project.write`; queries through `db`, across spaces. *(recommended)*
- B. Everything on `db`, with an object reference `{ space, id }`.

Recommendation: A. An object id is unique only in its space, every write is one change in one space, and every right is per space, so the space is the natural receiver. Queries span spaces because RFC 0006 decided one database per member for exactly that.

Decided (Carsten, 2026-11-09): the recommendation, (A).

**S3. What a write is.**

- A. `space.write(tx => ...)`: a synchronous recorder whose writes become one change, resolved when folded on the device; shorthands for one call; `db.sync.confirmed` for the relay. *(recommended)*
- B. An async callback that may read inside the transaction.
- C. Only single calls, each one change.

Recommendation: A. It maps one call to one change, so the app sees exactly what is atomic. B suggests an isolation that does not exist: a fold may run between the reads, and `replaces` is computed when the change is sealed. C makes "create a task and link it" two changes that another member may receive apart.

Decided (Carsten, 2026-11-09): the recommendation, (A).

**S4. Field operations.**

- A. Plain values for basic values and links, `null` to clear, `nc.add` and `nc.remove` for sets and many links. *(recommended)*
- B. As A, and also a whole array for a set, which the SDK turns into additions and removals.

Recommendation: A. B removes concurrent additions its author never saw, which breaks RFC 0004's add wins for the developer who expects "exactly these".

Decided (Carsten, 2026-11-09): the recommendation, (A).

**S5. The shape of a read.**

- A. A plain object as `nc.Infer` types it, with `$id`, `$space`, `$deleted`, `$conflicts` and `$raw`; a choice may hold `nc.Unknown`; a value outside the field's kind reads `undefined` and is named in `$raw`; `space.inspect` gives every detail. *(recommended)*
- B. Every field wrapped as `{ value, conflicts }`.
- C. As A, but a value outside the kind is returned as it is, typed `unknown`.

Recommendation: A. Unknown options are common after an additive change (RFC 0005, D5), so the type makes the app handle them. Values outside the kind come only from buggy or hostile members; typing every field as possibly anything would burden every read for a rare case, and `$raw` still makes it visible (principle 3). It amends what RFC 0005 says of `nc.Infer` for choices; see [Conflicts](#conflicts-with-genesismd-and-the-other-rfcs).

Decided (Carsten, 2026-11-09): the recommendation, (A).

**S6. The public names of RFC 0004's reads.**

- A. Keep RFC 0004's working names: `value`, `conflicts`, `lostRemovals`, `outcome` with `won`, `lost`, `replaced`, `applied`, and the statuses `ok`, `deleted`, `missing`, with `wrong collection` as `wrongCollection`. *(recommended)*
- B. Other names, such as `losers` for conflicts and `dangling` for missing.

Recommendation: A. They are plain words, and the RFCs and the FAQ already use them.

Decided (Carsten, 2026-11-09): the recommendation, (A).

**S7. How a query is written.**

- A. Handles from the model: `model.tasks.indexes.byStatusPriority.equal("open").take(50)`, passed to `db.find`. *(recommended)*
- B. An options object: `db.find(model.tasks, { index: "byStatusPriority", equal: ["open"], limit: 50 })`.

Recommendation: A. It is the chain RFC 0001 shows for views, TypeScript checks the equality values against the index, and a view will be queried the same way.

Decided (Carsten, 2026-11-09): the recommendation, (A).

**S8. A query in one space.**

- A. An index option `scope: "space"` that puts the space id first, and `space.find`; RFC 0005 and RFC 0006 are amended. *(recommended)*
- B. No query per space in v1: an app filters the cross-space page in memory.
- C. Every index is per space, and `db.find` queries each space and merges.

Recommendation: A. "The open tasks of this project" is the first screen of most apps. B reads rows it throws away, which principle 6 rules out. C loses what RFC 0006's decision 1 chose one database for, a page across spaces in one range lookup.

Decided (Carsten, 2026-11-09): the recommendation, (A).

**S9. Deleted objects in queries.**

- A. Queries skip deleted objects; `space.get` returns them with `$deleted: true`; RFC 0006 is amended so the index does not hold them. *(recommended)*
- B. Queries return them, flagged; the app filters.

Recommendation: A. An app almost never lists deleted objects, and filtering in the app reads rows it throws away. A deletion can be undone ([RFC 0004, decision 4](0004-merge-rules-basic-types.md#decisions-for-the-owner)), so `get` still finds the object to restore it.

Decided (Carsten, 2026-11-09): the recommendation, (A).

**S10. Subscriptions.**

- A. `db.subscribe(query, listener)` and `space.subscribe` pass the whole fresh page or object; `db.changes.subscribe` passes the ids each fold touched; each returns a function that stops it. *(recommended)*
- B. Only a stream of changes; the app re-reads.

Recommendation: A. Re-running the page is what RFC 0006's notification is for, and an app that gets the whole page cannot apply a diff wrongly. B makes every app write the same loop.

Decided (Carsten, 2026-11-09): the recommendation, (A).

**S11. Space names.**

- A. The SDK has none; the app keeps names in its own model. *(recommended)*
- B. A reserved core collection in each space, such as `$space` with `name` and `description`.

Recommendation: A for v1. B needs a collection that is not in the app's lock, which RFC 0002's fold step 7 parks as unknown, so it needs amendments to RFC 0002 and RFC 0005. An app already models what a project is; its name belongs there. B can come later as an addition.

Decided (Carsten, 2026-11-09): the recommendation, (A).

**S12. Invitations.**

- A. One hop: the joining member creates its invitation, its public identity, and the app carries it to an admin, who adds it with a role. *(recommended)*
- B. Two hops: the admin creates an invitation with the space and the role, the joining member accepts it and sends its identity back.
- C. The relay carries invitations and answers, in a mailbox.

Recommendation: A. It is what RFC 0003 and use case 3 show, `members.add(invitation, { role })`, and one channel is enough, since nothing in it is secret. B needs two trips through the app's channel. C adds relay messages (an amendment to RFC 0007) and lets the relay see who is about to join which space.

Decided (Carsten, 2026-11-09): the recommendation, (A).

**S13. Role names.**

- A. `"reader"`, `"writer"`, `"admin"`. *(recommended)*
- B. `"viewer"`, `"editor"`, `"owner"`.

Recommendation: A. They say what the role may do; RFC 0008 and the FAQ use them. "Owner" suggests one person, while a space has many admins.

Decided (Carsten, 2026-11-09): the recommendation, (A).

**S14. Groups.**

- A. No group object in v1: `members.add` takes a list, and the app keeps its own lists. *(recommended)*
- B. A group list stored only on the admin device that made it.
- C. Wait for the groups RFC before the SDK offers anything.

Recommendation: A. RFC 0008 defers groups, and a list that lives only on one device (B) is lost with it, and other admins cannot see it. A delivers what v1 promises, rights changed without moving data, with the app's own list; it partly differs from GENESIS.md, see [Conflicts](#conflicts-with-genesismd-and-the-other-rfcs).

Decided (Carsten, 2026-11-09): the recommendation, (A).

**S15. Accounts.**

- A. Name the entry points now, `db.account.pair`, `nc.open({ pairing })` and `db.account.removeMember`, and design them in the accounts RFC. *(recommended)*
- B. Leave accounts out of this RFC.

Recommendation: A. An agent needs a certificate from its account before it can join a space, so the rest of the API refers to pairing; naming it now keeps the names consistent.

Decided (Carsten, 2026-11-09): the recommendation, (A).

**S16. Relays.**

- A. One relay per database, given to `nc.open`. *(recommended)*
- B. A relay per space.

Recommendation: A for v1. Moving a space between relays is not designed ([RFC 0007, "What this RFC does not cover"](0007-relay.md#what-this-rfc-does-not-cover)). B can be added as an option of `db.spaces.create` later.

Decided (Carsten, 2026-11-09): the recommendation, (A).

**S17. Errors.**

- A. One class, `nc.Error`, with a string `code` and ids. *(recommended)*
- B. A class per error.

Recommendation: A. Codes are easy to match on in both SDKs, map one to one to Rust's `ErrorCode`, and keep the API surface small.

Decided (Carsten, 2026-11-09): the recommendation, (A).

**S18. The Rust SDK in v1.**

- A. The same calls in snake case; fields read and written by name, with `Value`. *(recommended)*
- B. Typed structs generated from the lock in v1.

Recommendation: A. RFC 0005 leaves code generation for later; A works for every lock today, and B can be added without changing A.

Decided (Carsten, 2026-11-09): the recommendation, (A).

## Conflicts with GENESIS.md and the other RFCs

This RFC follows GENESIS.md's principles: reads never touch the network, queries run only through declared indexes, developers never handle a key, every permission change returns a notice, and the API is the same on every target. It changes no other RFC. These points need a change elsewhere, which this RFC does not make; pm routes each one to the ticket that edits that file.

1. **RFC 0005 and RFC 0006, queries in one space** (S8). An index ends with the space id, so no query can fix a space. `scope: "space"` needs an option in `nc.index` and a key in the lock (RFC 0005, "Indexes" and "Public names") and a column order in RFC 0006, "Declared indexes".
2. **RFC 0006, deleted objects in indexes** (S9). RFC 0006 does not say whether a deleted object's row stays in an index. Under S9 A it does not, for example through a partial index.
3. **RFC 0005, `nc.Infer` for choices** (S5). RFC 0005 types a choice as the union of its options; S5 adds `nc.Unknown`.
4. **RFC 0005, personal field without a person.** RFC 0005 is silent on a personal field whose person link is unset. This RFC proposes `noPerson`; RFC 0005's "What the build checks" could say so.
5. **RFC 0003, "How the SDK hides keys".** Its sketch uses the role `"editor"`, which is not one of RFC 0008's roles, and `db.spaces.create({ name: "Project A" })`, where the name has no place (S11). Use case 3 uses `"editor"` too.
6. **RFC 0001 and use case 4, `db.find` on a view.** The name stays, but views are not designed yet; until they are, the examples can only run on indexes.
7. **Use cases 6, 7 and 8.** `db.get(model.proposals, id)` reads through `db` instead of a space (S2); `db.subscribe(model.messages, { where: { to: me, read: false } }, ...)` filters without a declared index, which principle 6 rules out (S7); `db.insert` is `space.create` here; `nc.useLive` is not proposed. Use case 1 marks personal fields with `nc.string({ personal: true })`, which RFC 0005 decided as `.personal({ person })`.
8. **GENESIS.md, groups.** GENESIS.md says a group is "a list of members that the SDK expands". Under S14 A the SDK expands a list the app passes, but keeps no list of its own, so removing a person from a group is the app's loop over its spaces.
9. **RFC 0003, the source of a copy.** RFC 0003 says copies in an ad-hoc space keep a reference to their source and does not say where; this RFC keeps it on the parent only.
10. **RFC 0008, invitations.** RFC 0008 leaves out how a new member's identity reaches an admin; S12 proposes it. RFC 0008's "What this RFC does not cover" could link here once decided.

## How this RFC is tested

`scripts/rfc-0009.test.mjs`, run by `repo:test` in `pnpm gates`, checks that this file exists, links to RFC 0001 to 0008 and GENESIS.md, that every relative link and its anchor resolve, that it has a section for every part NOC-64 names, that every decision has options and a recommendation, that every SDK name its TypeScript examples use is listed under [Public names](#public-names) and every row there links the RFC it builds on, that it shows the Rust SDK, and that the [RFC index](README.md) lists it. The design itself is tested when it is built: each call against the convergence harness and the view, natively and in the browser.
