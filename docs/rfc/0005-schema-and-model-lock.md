# RFC 0005: The schema language and model.lock.json

REQUEST FOR COMMENTS · NOVEMBER 2026 · Ticket NOC-11

This RFC defines how a developer describes the model of an app once, in TypeScript, and the versioned `model.lock.json` that `ncdb model build` produces from it. The Rust core, the TypeScript SDK and the Rust SDK all read `model.lock.json`; none of them reads the TypeScript.

It starts from the example in [RFC 0001, "The design in one page"](0001-nocentraldb.md#the-design-in-one-page), and follows the principles in [GENESIS.md](../../factory/GENESIS.md). It builds on [RFC 0002](0002-change-and-log-format.md) (the change and the log format), [RFC 0003](0003-keys-and-cryptography.md) (keys and cryptography) and [RFC 0004](0004-merge-rules-basic-types.md) (merge rules for basic values, sets and links). [RFC 0006](0006-local-view.md) (the local view) turns the collections, fields and indexes declared here into SQLite tables. Where this RFC differs from RFC 0001, it says so in [Where this differs from RFC 0001](#where-this-differs-from-rfc-0001).

**Status.** Decided. GENESIS.md reserves public API names and on-disk formats for the owner, and the schema functions are the first thing a developer sees. The owner accepted the recommendation of every decision this RFC asked: D1 to D15 (Carsten, 2026-11-08), D16 and D17, which building `ncdb model build` raised (Carsten, 2026-11-09), and D18 to D22 on references into another space (Carsten, 2026-11-10). The design below describes those choices, and [Decisions](#decisions) records each one. Every name this RFC introduces is listed in [Public names](#public-names). [History](#history) says when each part was written.

**How to give feedback.** This RFC has no discussion of its own yet: start one in the category [RFC feedback](https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback), named after the RFC, or open a pull request against this file to change its text. [Giving feedback](README.md#giving-feedback) says how, and what happens to your point.

## What this RFC does not cover

- **Computed fields and views.** This RFC names the places where they go in the schema and in `model.lock.json`, so their names are reserved, but it does not design them. A later RFC does. Indexes are declared here (see [Indexes](#indexes)); how a query uses them is RFC 0006's.
- **Rich text and streams.** The agent task board needs a rich-text body, so this RFC reserves the names `nc.richText` and `nc.stream`. Their field types, and so their entries in `model.lock.json`, belong to later RFCs.
- **Writing a field type plugin.** This RFC says how a schema uses a plugin's field type and how `model.lock.json` references it. How a plugin is written and loaded belongs to the plugin RFC, under the contract of RFC 0004.
- **The crypto of personal fields.** This RFC says how the schema marks a field as personal and ties it to a person. RFC 0003 encrypts it.
- **Migrations.** A change that is not additive is refused (see [The additive-change rule](#the-additive-change-rule)). That includes moving a field to a new rule version of its type, which RFC 0004 allows only with a migration. Migrations are a later RFC (decision D12).

## Terms

| Term | Meaning in this RFC |
| --- | --- |
| Schema | The TypeScript module in which a developer describes the model, with the functions of `@nocentraldb/sdk`. |
| Model | What the schema describes: the collections, their fields and each field's type. |
| Lock | The file `model.lock.json`, which `ncdb model build` writes from the schema and the previous lock. The core and both SDKs read only the lock. |
| Collection | A named kind of object, such as `tasks`. Every object belongs to exactly one collection and one space. |
| Field | A named, typed value of an object, such as `title`. Each field has exactly one field type. |
| Field type | How a field stores its value and merges concurrent changes (RFC 0004), identified by a type id and a rule version. |
| Model version | A whole number that `ncdb model build` raises by one each time the model changes. |
| Old client | An app or agent whose lock has a lower model version than the lock that wrote some of the changes it receives. |

## The schema in TypeScript

A developer writes the schema once, as a TypeScript module that exports a model. This is the schema of the agent task board (RFC 0001, [Example applications](0001-nocentraldb.md#example-applications)), at model version 2:

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
      body: nc.richText(),
      status: nc.oneOf(["open", "in_progress", "done"]),
      priority: nc.int(),
      estimate: nc.float(),
      archived: nc.boolean(),
      tags: nc.set(nc.string()),
      assignee: nc.link("people"),
      notesAboutAssignee: nc.string().personal({ person: "assignee" }),
      dependsOn: nc.links("tasks"),
    },
    {
      computed: { blocked: nc.reach("dependsOn", { status: ["open", "in_progress"] }) },
      views: { ready: nc.view("tasks", { where: { blocked: false }, sort: ["priority"] }) },
      indexes: {
        byStatusPriority: nc.index(["status", "priority"], { equal: 1 }),
        byPriorityDesc: nc.index([{ field: "priority", direction: "desc" }]),
      },
    },
  ),
});

export type Task = nc.Infer<typeof model.tasks>;
```

### Collections

`nc.model({ ... })` takes the collections, keyed by name. `nc.collection(fields, options)` takes the fields, keyed by name, and an optional second argument with the places for computed fields (`computed`), views (`views`) and indexes (`indexes`).

The schema says nothing about spaces. Any object of any collection can live in any space. Which space an object goes into is decided when the app creates it, not in the model, because the same model serves many spaces (one per project, one per shared document).

### Names

Collection and field names are the names that changes carry forever: RFC 0002 names the collection and the field of every operation by these names, not by numbers. A name is therefore part of the stored data, not just of the code.

- A name matches `^[a-z][A-Za-z0-9]{0,62}$`: it starts with a lower-case letter and has at most 63 characters, ASCII letters and digits only.
- Two names in the same scope may not differ only in case (`title` and `Title`), because the local view is SQLite, whose column names ignore case.
- A collection's fields, computed fields, views and indexes share one scope: a computed field cannot have the name of a stored field.
- Names that start with `_` or `$` are reserved for the core, which the pattern above already excludes.

The TypeScript key is the stored name. There is no alias in v1 (see decision D8).

### Field types

Each function returns a field. The functions map to the field types of [RFC 0001, "Field types we ship"](0001-nocentraldb.md#field-types-we-ship) and to the merge rules of RFC 0004:

| Function | Field type in the lock | Merge rule (RFC 0004) | Read value |
| --- | --- | --- | --- |
| `nc.string()` | `ncdb/string` | Basic value: the last writer wins, losing writes stay as conflicts | A string, NFC-normalised by the SDK on write |
| `nc.int()` | `ncdb/int` | Basic value | A signed 64-bit integer |
| `nc.float()` | `ncdb/float` | Basic value | An IEEE 754 binary64 number |
| `nc.boolean()` | `ncdb/boolean` | Basic value | `true` or `false` |
| `nc.oneOf([...])` | `ncdb/choice` | Basic value | One of the listed options, or an unknown option kept as written |
| `nc.set(element)` | `ncdb/set` | Set: add wins, lost removals stay readable | The elements, in the order of their canonical bytes |
| `nc.link("collection")` | `ncdb/link` | Basic value whose value is an object id | The target, with its resolution status |
| `nc.links("collection")` | `ncdb/links` | Set of object ids | The targets, each with its resolution status |
| `nc.ref("collection")` | `ncdb/ref` | Basic value whose value is a space id and an object id ([RFC 0004, "References"](0004-merge-rules-basic-types.md#references)) | The target's space id and object id; its state, ok, not accessible or missing, comes from the member's own copies |
| `nc.richText()` | reserved | Later RFC (Yjs and Yrs) | Later RFC |
| `nc.stream()` | reserved | Later RFC | Later RFC |

`nc.set` takes one of the basic value functions as its element: `nc.set(nc.string())`, `nc.set(nc.oneOf(["bug", "feature"]))`. A set of links is `nc.links`, not `nc.set(nc.link(...))`.

`nc.link` and `nc.links` take one collection name, or a list of them when a link may point into several collections: `nc.links(["tasks", "milestones"])`. A link always points into the object's own space (RFC 0004).

A field type from a plugin is used the same way: the plugin's package exports a function that returns a field, and the schema calls it next to the built-in ones.

```ts
import { counter } from "@acme/ncdb-counter";

export const model = nc.model({
  tasks: nc.collection({ votes: counter() }),
});
```

### References into another space

Rights come from many small spaces (GENESIS.md), so related data often lives in different spaces, where a link cannot reach. An app keeps its customers in one space and their orders in another, so that a member who handles orders does not read the customers. A reference, `nc.ref`, names an object that may live in another space:

```ts
import { nc } from "@nocentraldb/sdk";

export const model = nc.model({
  customers: nc.collection({
    name: nc.string().personal({ person: "self" }),
  }),
  orders: nc.collection(
    {
      customer: nc.ref("customers"),
      total: nc.float(),
    },
    { indexes: { byCustomer: nc.index(["customer"], { equal: 1 }) } },
  ),
});
```

`nc.ref` takes one collection name, or a list of them, as `nc.link` does: `nc.ref(["customers", "leads"])`. It stores the target's space id and object id, and merges as a basic value: the last writer wins ([RFC 0004, "References"](0004-merge-rules-basic-types.md#references)). The lock writes it as a link, with its own type id:

```json
{ "customer": { "rule": 1, "since": 1, "targets": ["customers"], "type": "ncdb/ref" } }
```

`byCustomer` finds all orders of one customer in one equality lookup (see [Indexes](#indexes)).

On read, the SDK resolves a reference from the member's own copies to one of three states: ok, not accessible (the member is not in the target's space) or missing (not arrived, or deleted). The state differs from member to member, so nothing stored depends on it (principle 2): computed fields and views never follow a reference, no index holds its state, and the read object does not carry it ([Types for the app](#types-for-the-app)). A subscription re-runs when the referencing object changes, not when the target arrives, changes or is deleted, and not when the member joins or leaves the target's space. An app that shows the target reads it, and subscribes to it, in the target's space.

A reference gives no access to the target's space, and keeps no referential integrity. [RFC 0004, "What this does not do"](0004-merge-rules-basic-types.md#what-this-does-not-do) states its limits.

A reference takes `.describe(` and `.deprecated(`. It cannot be `.personal(`, and cannot be the `person` of a personal field: a personal field is sealed under its person's key in its own space ([RFC 0003, "The person key"](0003-keys-and-cryptography.md#the-person-key)), and a person who lives in another space has no key there that a shred could destroy. There is no field of many references in v1: `nc.set` takes basic values only, and there is no `nc.refs`.

### No required fields and no defaults

Every field may be unset. A field nobody has written reads as unset (`undefined` in TypeScript), whatever its type. There is no `required` and no default value in v1.

The reason is the additive-change rule. An object written by an old client never has the fields a newer model added, so a newer client must be able to read "unset" for every field anyway. A default value would also have to be the same on every client forever, or two clients would read different values from the same changes. See decision D6.

### Modifiers

A field can carry three modifiers, chained after the field function:

- `.personal({ person })` marks the field as personal and ties it to a person (see [Personal fields](#personal-fields)).
- `.describe(text)` adds a description. It is copied into the lock, where an agent reading the model finds it. It changes nothing else.
- `.deprecated(reason)` hides the field from the SDK's TypeScript types and from writes, but keeps it in the model and in the data (see [The additive-change rule](#the-additive-change-rule)). It chains on `nc.collection(...)` too, and deprecates the whole collection the same way: the SDK hides the collection from its types and refuses writes into it.

### Computed fields and views: named, not designed

The second argument of `nc.collection` has two places, `computed` and `views`, as in RFC 0001. `nc.reach` and `nc.view` are the names RFC 0001 introduced for one computed function and for a view. This RFC records only the names of computed fields and views in the lock, so that they take a name in the collection's scope. Their arguments and their entries in the lock are left to the computed-fields RFC.

### Indexes

GENESIS.md's principle 6 says queries run only through declared indexes. The schema declares them in the `indexes` place of `nc.collection`, by name:

- `nc.index(fields, options)` takes the fields in index order. Each is a field name, which sorts ascending, or `{ field, direction }` with `direction` `"asc"` or `"desc"`.
- The option `equal` is the number of leading fields a query must fix with equality, 0 by default. `byStatusPriority` above has `equal: 1`: a query on it names one status and pages through that status's tasks by priority.
- The option `scope: "space"` puts the space id before the index's fields, so that a query on it runs in one space, through `space.find` ([RFC 0009, "Queries and indexes"](0009-sdk-api.md#queries-and-indexes), decision S8). Without it, an index spans every space of the member. Where the space id goes among the index's columns is RFC 0006's.
- An index can use basic values (`nc.string`, `nc.int`, `nc.float`, `nc.boolean`, `nc.oneOf`), single links (`nc.link`) and references (`nc.ref`). A reference indexes as one column, the target's space id followed by its object id, as stored and whatever its state, so the index is the same on every member of the space ([RFC 0006, "Declared indexes"](0006-local-view.md#declared-indexes)). Sets, many links, rich text, streams and plugin types cannot be indexed in v1: a field with many values, or with no order of its own, needs its own rule for which rows it puts in an index. Computed fields in indexes belong to the computed-fields RFC.
- A personal field can be indexed. The view holds it in plaintext on the device anyway, and erases it from the view and its indexes after a shred (RFC 0006).

Everything else about an index is [RFC 0006, "Declared indexes and ordered pages"](0006-local-view.md#declared-indexes-and-ordered-pages): every index ends with the space id and the object id so its order is total, how values compare (text by its UTF-8 bytes, unset first in ascending order), the one query shape, the check that every query uses its index, and keyset cursors.

An index is local: it is built from the view on each device and never travels in a change. Adding, changing or removing an index therefore affects only the app that ships the lock, never another member (see [Accepted](#accepted)).

### Types for the app

`nc.Infer<typeof model.tasks>` is the TypeScript type of a task as the app reads it: every field optional; a choice as the union of its options and `nc.Unknown`, `{ unknown: rawValue }`, for an option the lock does not list yet; a link as an object id; and a reference as `{ space, id }`, the target's space id and object id, in the form `$space` and `$id` carry them ([RFC 0009, "Get by id"](0009-sdk-api.md#get-by-id)). It types an `nc.int` field as a `number`: a write must be a safe integer, between −(2^53 − 1) and 2^53 − 1, or the SDK refuses it with `invalidValue`. A stored value beyond that range, which another member or the Rust SDK can write, reads `undefined` and is named in `$raw`, and `space.inspect` returns it exactly, as a `bigint`. A set of ints that holds such an element reads the same way, as a whole. How reads expose conflicts, lost removals, a link's status and a reference's state is [RFC 0009's](0009-sdk-api.md#conflicts-lost-removals-and-link-states): `space.inspect` gives a reference's state as `status`, `"ok"`, `"notAccessible"` or `"missing"`.

## The structure of model.lock.json

`ncdb model build` reads the schema and the previous `model.lock.json`, checks that the new model is an additive successor of the previous one, and writes the new lock. The lock is committed to the app's repository next to the schema, and ships with the app.

```sh
ncdb model build            # reads ./model.ts, writes ./model.lock.json
ncdb model build --model src/model.ts --lock src/model.lock.json
ncdb model check            # in CI: fails if the lock is out of date or the change is not additive
```

### An example

This is the lock of the task board above, at model version 2. Version 1 had only `tasks` with `title`, `body`, `status`, `priority` and `dependsOn`, the computed field, the view and the index `byStatusPriority`. The rich-text body is left out of the example, because its entry belongs to the rich-text RFC.

```json
{
  "collections": {
    "people": {
      "fields": {
        "email": {
          "personal": { "person": "self" },
          "rule": 1,
          "since": 2,
          "type": "ncdb/string"
        },
        "name": {
          "personal": { "person": "self" },
          "rule": 1,
          "since": 2,
          "type": "ncdb/string"
        }
      },
      "since": 2
    },
    "tasks": {
      "computed": {
        "blocked": { "since": 1 }
      },
      "fields": {
        "archived": { "rule": 1, "since": 2, "type": "ncdb/boolean" },
        "assignee": {
          "rule": 1,
          "since": 2,
          "targets": ["people"],
          "type": "ncdb/link"
        },
        "dependsOn": {
          "rule": 1,
          "since": 1,
          "targets": ["tasks"],
          "type": "ncdb/links"
        },
        "estimate": { "rule": 1, "since": 2, "type": "ncdb/float" },
        "notesAboutAssignee": {
          "personal": { "person": "assignee" },
          "rule": 1,
          "since": 2,
          "type": "ncdb/string"
        },
        "priority": { "rule": 1, "since": 1, "type": "ncdb/int" },
        "status": {
          "options": ["open", "in_progress", "done"],
          "rule": 1,
          "since": 1,
          "type": "ncdb/choice"
        },
        "tags": {
          "of": { "type": "ncdb/string" },
          "rule": 1,
          "since": 2,
          "type": "ncdb/set"
        },
        "title": {
          "description": "One line; what has to be done.",
          "rule": 1,
          "since": 1,
          "type": "ncdb/string"
        }
      },
      "indexes": {
        "byPriorityDesc": {
          "equal": 0,
          "fields": [{ "direction": "desc", "field": "priority" }],
          "since": 2
        },
        "byStatusPriority": {
          "equal": 1,
          "fields": [
            { "direction": "asc", "field": "status" },
            { "direction": "asc", "field": "priority" }
          ],
          "since": 1
        }
      },
      "since": 1,
      "views": {
        "ready": { "since": 1 }
      }
    }
  },
  "format": 1,
  "model": {
    "hash": "sha256:a59fed6588bc432d392c671b9ce7816babffc1fd7208187e4f2dd2c952e0531d",
    "id": "6f1c0e2a9b7d4c3e8a5f0b1d2c3e4f5a",
    "version": 2
  },
  "plugins": {}
}
```

The hash in the example is computed from the example as shown, without the rich-text body.

### The keys

At the top:

| Key | Meaning |
| --- | --- |
| `format` | The version of the lock's own format. This RFC defines format 1. A reader that does not know the format refuses the lock, because the lock ships with the app and a mismatch is a build error, not data to keep. A later format is a superset of the previous one, and every SDK reads every earlier format. |
| `model.id` | 128 random bits in hex, drawn by the first `ncdb model build` of the app and never changed. It names the model's lineage, so the SDK can refuse a lock that is not a successor of the one it last opened (see [Two checks: at build and on the device](#two-checks-at-build-and-on-the-device)). |
| `model.version` | The model version: 1 for the first build, one higher for each build that changes the model. A build that changes only descriptions keeps the version. |
| `model.hash` | `sha256:` and the SHA-256 hash of the canonical JSON (RFC 8785) of `format`, `plugins` and `collections`, with every `description` removed. It identifies the model's content: two locks with the same hash describe the same model. It is a fingerprint, not a security feature. |
| `plugins` | The plugins whose field types the model uses, by namespace (see [How a field type is referenced](#how-a-field-type-is-referenced)). |
| `collections` | The collections, by name. |

Per collection: `since` (the model version that added it), `fields`, `computed` and `views` (names only, each with its `since`), `indexes`, and `deprecated` and `deprecatedReason` when the collection is deprecated, as for a field.

Per index: `fields`, an array in index order of `{ "field", "direction" }` with `direction` always written out as `"asc"` or `"desc"`; `equal`, the number of leading fields a query fixes, always written out; `scope`, written as `"space"` for an index with `scope: "space"` and left out for an index that spans every space; and `since`.

Per field:

| Key | Meaning |
| --- | --- |
| `type` | The field type id, such as `ncdb/string` or `acme/counter`. |
| `rule` | The version of the field type's merge rule (RFC 0004, "The plugin contract"). |
| `since` | The model version that added the field. |
| `options` | For `ncdb/choice`: the options, in the order the schema lists them. |
| `of` | For `ncdb/set`: the element's type, with `options` when the element is a choice. |
| `targets` | For `ncdb/link`, `ncdb/links` and `ncdb/ref`: the collections a target may be in. |
| `personal` | For a personal field: `{ "person": "self" }` or `{ "person": "<link field>" }`. |
| `description` | The text of `.describe()`. |
| `deprecated` | The model version that deprecated the field, and `deprecatedReason`, the text of `.deprecated()`. |
| `config` | For a plugin's field type: its options, as the plugin defines them. |

### Canonical form

`ncdb model build` writes the lock in one form only: keys sorted by their UTF-16 code units (as RFC 8785 sorts them), two spaces of indentation, LF line endings, a final newline, and short objects on one line as a formatter would put them. Two builds of the same schema on the same previous lock give the same bytes, so `ncdb model check` can compare bytes, and a pull request shows only what changed.

### How a field type is referenced

A field type is referenced by its **type id** and its **rule version**:

- **Built-in types** have ids in the namespace `ncdb/`, which is reserved: `ncdb/string`, `ncdb/int`, `ncdb/float`, `ncdb/boolean`, `ncdb/choice`, `ncdb/set`, `ncdb/link`, `ncdb/links`, `ncdb/ref`. Rule version 1 is RFC 0004's rule for each.
- **Plugin types** have ids `<namespace>/<name>`, where the namespace is a key of `plugins`. The entry names the plugin's packages, so every SDK knows what to load:

```json
{
  "plugins": {
    "acme": { "crate": "acme-ncdb-counter", "npm": "@acme/ncdb-counter" }
  }
}
```

  A field then reads `{ "type": "acme/counter", "rule": 2, "since": 3 }`. The package version is not in the lock: the type id and the rule version are the contract (RFC 0004: a merge rule never changes under the same type id and rule version), and any package version that implements that rule will do.

A client must run the exact rule version the lock names. Because the build refuses to move an existing field to another rule version (see the next section, and decision D12), an app that loads its own lock always has the rule for every field in it.

## The additive-change rule

GENESIS.md gives up "free schema changes (rename, retype, delete a field)" so that clients that stay offline for weeks keep working, with "a check that refuses a model old clients cannot read". This section is that check. `ncdb model build` compares the new model with the previous lock and accepts a change only when every client that runs the previous lock can still store, forward and keep all data written under the new one, and every client that runs the new lock can read all data written under the previous one.

### Accepted

| Change | Why old clients still work |
| --- | --- |
| Add a collection | Old clients park operations on it (RFC 0002) and fold them after an upgrade. |
| Add a field to a collection | The same. Objects written before read the new field as unset. |
| Add a personal field | As adding a field. |
| Append an option to a choice, or to a set of choices | Old clients keep the new option as written and read it as an unknown option (RFC 0004). |
| Add a collection to a link's or a reference's `targets` | Old clients keep the link and read its status as **wrong collection** (RFC 0004), and keep the reference and read its state as **missing** ([RFC 0004, "References"](0004-merge-rules-basic-types.md#references)), until they upgrade. |
| Add a computed field or a view (name only, for now) | Computed values are derived locally; they are not in the changes. |
| Add, change or remove an index, or change its scope | Indexes are built locally from the view (RFC 0006) and never travel in a change. A page cursor names its index and the model hash (RFC 0006), so a cursor from before the change is refused, not misread. |
| Add, change or remove a description | Descriptions change no data. The model version stays. |
| Deprecate a field or a collection | It stays in the lock and in the data. The SDK stops offering it to the app. |
| Use a new plugin type for a new field | As adding a field. |

### Refused

| Change | Why it is refused |
| --- | --- |
| Remove or rename a collection or a field | Changes carry names. Data written under the old name would belong to nothing; an old client would keep writing the old name. Deprecate instead. |
| Change a field's type | Two clients would merge the same changes by different rules and diverge (principle 2). |
| Move a field to a new rule version of its type | RFC 0004 allows a new rule version only with a migration that every member runs before the schema uses it. Migrations are a later RFC; until it exists, the build refuses (decision D12). A new field can use the new rule version at once. |
| Remove, rename or reorder an option of a choice | Stored values name options; order may become sort order in views. |
| Remove a collection from a link's or a reference's `targets` | Existing links would turn from **ok** to **wrong collection**, and references from **ok** to **missing**, on upgraded clients only. |
| Mark an existing field as personal | The field's earlier values are already in the log in plain form, under the space key only. Destroying a person key would not erase them, and a reader would believe it did (principle 4). Add a new personal field instead. |
| Unmark a personal field, or change its `person` | Old values stay sealed under person keys; new ones would not be. The field would be half erasable. |
| Undeprecate a field | Old clients that stopped writing it would not write it again. Add a new field instead. (Decision D7.) |
| An index on a field that cannot be indexed, on an unknown field, or with an `equal` larger than its number of fields | The core could not build it, or no query could use it. |
| A collection or field name that breaks the naming rules, or collides in case with another name in its scope | The local view could not store it. |
| A lock with another `model.id` or a lower `model.version` than the previous one | It is not a successor of the previous model. |

### An example of each

Version 2 above adds `people`, five fields of `tasks`, and keeps everything of version 1. `ncdb model build` accepts it:

```text
$ ncdb model build
model.lock.json: model version 1 -> 2
  + collection people (2 personal fields)
  + tasks.archived    ncdb/boolean
  + tasks.assignee    ncdb/link -> people
  + tasks.notesAboutAssignee ncdb/string, personal to tasks.assignee
  + tasks.estimate    ncdb/float
  + tasks.tags        ncdb/set of ncdb/string
  + index tasks.byPriorityDesc (priority desc)
additive: clients on model version 1 keep every change written under version 2.
```

A developer then changes `priority` from `nc.int()` to `nc.oneOf(["low", "high"])`, and renames `title` to `summary`. `ncdb model build` refuses and writes nothing:

```text
$ ncdb model build
model.lock.json: refused, the change is not additive (model version stays 2)
  x tasks.priority: type changes from ncdb/int to ncdb/choice.
    Clients would merge and read existing values differently.
    Add a new field instead, and deprecate tasks.priority.
  x tasks.title: removed. Changes name fields; data in tasks.title would belong to nothing.
    To stop using it, keep it in the schema with .deprecated("..."). To rename it, add tasks.summary.
```

### Two checks: at build and on the device

`ncdb model build` only knows the previous lock in the repository. If a developer deletes the lock, or two branches each build version 3 and the merge keeps one, the build cannot see what shipped before. So the same check runs a second time, on every device:

- The SDK stores, in the local database, the lock it last opened the database with.
- When the app opens the database with a different lock, the SDK runs the additive check between the stored lock and the new one. It compares content, not just version numbers. If the new lock has another `model.id`, or is not an additive successor, the SDK refuses to open the database, with an error that names each refused change. It deletes and rewrites nothing.
- An app that is rolled back to an older version opens the database with an older lock. The SDK accepts that when the stored lock is an additive successor of the older one, and the app behaves as an old client: what it does not know stays in the database, as parked data.

Two branches that both raise the version conflict in `model.lock.json`, so git makes the developer resolve it; `ncdb model build` on the merged schema then writes the next version. A lock that shipped from a branch and was then dropped is caught by the device check.

## Data from a newer model

Old clients exist: an agent on a laptop that has been closed for three weeks, a phone that has not updated its app. They receive changes written under a newer model. Principle 3 says they keep what they do not understand yet. This RFC adds no new mechanism for it; it relies on the format of RFC 0002 and the rules of RFC 0004, and says how they meet:

1. **Changes write fields, never whole objects.** Every operation names one field of one object (RFC 0002). An old client that edits a task writes only the fields it knows; it cannot overwrite or drop a field it does not know, because it never writes the object as a whole. This is why the schema needs no "unknown fields" bucket.
2. **An operation on an unknown collection or field is parked.** The old client stores the whole change in its log, forwards it, folds the operations it knows, and parks each operation on an unknown collection or field on its own, as [RFC 0002, "The atomic unit"](0002-change-and-log-format.md#the-atomic-unit) settles. After an upgrade the core folds what it parked in stamp order, as if it had just arrived. Each field's state is a function of the set of its operations (RFC 0004), so the upgraded client reaches the same state as a client that had the new model all along.
3. **An unknown option is kept as written.** A choice, or a set of choices, holding an option the old lock does not list keeps the raw value; the app reads it as an unknown option (RFC 0004).
4. **A link into an unknown target collection is kept.** Its status reads **wrong collection** on the old client, and **ok** after the upgrade, with no change to the link.
5. **Values outside the schema are kept, never repaired.** A value of the wrong kind (a string in an `ncdb/int` field, written by a buggy or hostile member) is kept as written, and the app reads the raw value, as RFC 0004 does for an option the reader's schema does not list. RFC 0004's contract (C5) requires that such a value causes no panic and that the state still converges; this RFC adds no read status for it. The SDK reads such a field as unset and names it in `$raw`, and `space.inspect` gives the raw value ([RFC 0009, "Get by id"](0009-sdk-api.md#get-by-id)). The SDK never writes such a value.
6. **Parked data is visible.** The SDK reports, per space, how many changes and operations are parked and why (unknown collection, unknown field, unknown type, another model), and the highest model version among the parked changes and operations of the app's own model (decision D10). An app can show "Some data needs a newer version of this app" instead of hiding it. Changes parked as "another model" are counted apart, under their reason: their model version belongs to another lineage, and no upgrade of this app reaches it ([RFC 0002, "The writer's model"](0002-change-and-log-format.md#the-writers-model)).

The other direction is simpler. A newer client reading data written under an older model finds every field the older model did not have unset, which every field allows (see [No required fields and no defaults](#no-required-fields-and-no-defaults)).

What this does not do, stated plainly:

- An old client cannot read, query or index data in fields it does not know. Computed fields and views on an old client are computed from what it understands, so they can differ from an upgraded client's until it upgrades. This is RFC 0002's reading of principle 2, which the owner confirmed (Carsten, 2026-11-07): every client that understands all the changes it holds reaches the same state.
- A client that never upgrades never reads the new fields. It still keeps and forwards them.
- Deprecated fields stay in every copy and in every sync. Deprecation saves the developer from the field, not the disk.
- A change names the model that wrote it, but that stops honest and buggy apps, not a hostile member. A space records the `model.id` it was created with in its genesis ([RFC 0008, "Genesis"](0008-membership.md#genesis)), and the SDK refuses to write into a space of another model. Every sealed change carries its writer's `model.id` and model version in payload key 64 ([RFC 0002, "The writer's model"](0002-change-and-log-format.md#the-writers-model)), and a member parks, whole, every change whose key 64 names a `model.id` other than the one of its own `model.lock.json` (decision D13). So two apps with different `model.id`s that write into one space, with the same field name for fields of different types, cannot make members fold one field by two rules. A member that writes no key 64, or writes the space's own `model.id`, has its change folded by the rules of each member's lock; it gains nothing new, since a member can already write any value into any field.

## Personal fields

[RFC 0001, "Field types we ship"](0001-nocentraldb.md#field-types-we-ship) says a basic value can be marked as personal when the field is added, and tied to a person. This RFC defines the marking only; RFC 0003 defines the person key and the encryption, and RFC 0002 the sealed value that carries the person-key id.

A field is personal when the schema chains `.personal({ person })` to it. `person` says which person the field is about:

- `"self"`: the object is the person. `people.email` is about the person the `people` object stands for.
- The name of a link field (`nc.link`) of the same collection: the field is about the person that link points to. `tasks.notesAboutAssignee` holds notes about whoever `tasks.assignee` names.

### About whom, not by whom

A personal field is sealed under the key of the person it is **about**, never under the key of its author. `tasks.notesAboutAssignee` holds what someone wrote about the assignee, for example "prefers calls in the morning"; erasing the assignee erases those notes. The assignee's own writing is something else: a task title or a comment the assignee wrote about other things is not about the assignee, is not sealed under their key, and is not erased with them. What stays of the assignee as author is their member id in the header of each change they wrote (RFC 0002), so the audit trail still shows that a change came from them, without anything they were said to be.

Two things are not erasure, stated plainly:

- **Removing a member is not erasure.** Removing a member blocks their access to the space from then on (RFC 0003, a new space key generation). Their member key, the data they wrote and the personal fields about them all stay. Only a shred destroys a person key.
- **Being a member is not needed to be a person.** Most persons in an app never log in. They have no account and no member key; the app only holds data about them.

### A person who is not a user

A CRM keeps customers who never use the app:

```ts
export const model = nc.model({
  customers: nc.collection({
    name: nc.string().personal({ person: "self" }),
    address: nc.string().personal({ person: "self" }),
    birthDate: nc.string().personal({ person: "self" }).describe("ISO 8601 date, YYYY-MM-DD."),
    segment: nc.oneOf(["lead", "active", "former"]),
  }),
});
```

The customer has no account and no member key. `name`, `address` and `birthDate` are sealed under the customer's person key; `segment` is not personal and is not erased. When the customer asks, by email for example, to have their data erased, a user of the app starts the erasure as a work order ([GENESIS.md](../../factory/GENESIS.md), "Work orders"): one member performs the shred, and a second account signs it off, as [RFC 0003, "Account sign-off of a shred"](0003-keys-and-cryptography.md#account-sign-off-of-a-shred) requires. The API for starting and signing such a work order belongs to the work-order RFC and the SDK's API review, not to this RFC.

### Identifying the person

Within a space, the person is identified by the object id of the person's object. The SDK asks RFC 0003's person-key store for that person's key in the space and seals the value with it.

Across spaces this is not enough. Object ids belong to one space, so the same customer recorded in two spaces is two objects with two ids, while RFC 0003 shreds a person's key "in every space the account can reach". How the core knows that the two objects are one person is decision D14: a person id, which the core carries over when the app records the same person in another space, with `tx.create`'s option `samePersonAs` ([RFC 0009, "Personal fields"](0009-sdk-api.md#personal-fields)). The marking in the schema does not depend on it. The person need not be a member of the space; it is usually a customer, a patient or an employee the data is about.

### What the build checks

- `person` is `"self"`, or names a field of the same collection whose type is `ncdb/link`. A link field with several target collections is allowed; a reference is not.
- The link field named as `person` is not itself personal: it holds an object id, which the relay cannot read anyway, and the SDK needs it to find the key.
- In v1, only basic values can be personal: `nc.string`, `nc.int`, `nc.float`, `nc.boolean` and `nc.oneOf`. Sets, links and references cannot. A set compares its elements by their bytes (RFC 0004), and a sealed value has different bytes each time it is written, so a set of sealed values could not tell two equal elements apart. A link must stay readable to resolve its target, and a reference points into another space, where its target's person has no key ([References into another space](#references-into-another-space)). Rich text and streams decide for themselves in their RFCs. (Decision D9.)
- A field is personal from the version that adds it, or never (see [Refused](#refused)).

The build cannot check that the person link is set when a personal field is written. The SDK refuses such a write with `noPerson`: sealing the value under no person would make it unerasable, and choosing a person would guess ([RFC 0009, "Personal fields"](0009-sdk-api.md#personal-fields)).

### Two limits

Stated plainly:

- **The person is chosen when the value is written.** If `tasks.assignee` changes from Alice to Bob, a `notesAboutAssignee` written before the change stays sealed under Alice's key, and one written after under Bob's. Concurrent writes may seal under different persons. This converges, because each sealed value names its person-key id (RFC 0002), but erasing Bob does not erase the note Alice's key sealed.
- **Only marked fields are erasable.** Personal data in a field the schema does not mark, a task's `title` for example, is not encrypted with a person key, and erasure does not reach it. RFC 0001 says the same of unmarked free text. The next section says what an app does about it.

### Personal data in free text

NoCentralDB does not and cannot find a person's name inside free text. A title, a comment or a rich-text body that says "call Anna Schmidt about the invoice" holds Anna's name in plain form under the space key, and erasing Anna does not reach it (principle 4). Detecting names in text would need a guess, and a guess that misses a name would let a reader believe an erasure reached what it did not.

Keeping names out of free text is therefore the app's job (decision D15). An app whose free-text fields may name persons:

1. **Knows which fields those are:** comments, descriptions, bodies, anything a person or an agent types freely.
2. **Stores a reference to the person, not the name:** the person's object id, or their person id (D14), written into the text as a mention token where the user typed the name.
3. **Resolves the reference when it displays the text,** from the person's personal fields: the mention shows the person's current `name`.

After a shred, the reference no longer resolves to a name: the person's personal fields are erased, so the mention shows a placeholder such as "erased person", and no name was ever stored in the text. This is the recommended pattern, not a feature of the core: the core neither inserts mention tokens nor checks that an app uses them. What a mention token looks like is an idea for the rich-text RFC and the SDK's API review, not a format this RFC defines.

What the pattern does not cover: a name a user types without making it a mention, text pasted from elsewhere, and exports and screenshots. Those stay as written.

## Where this differs from RFC 0001

The example in [RFC 0001, "The design in one page"](0001-nocentraldb.md#the-design-in-one-page) is kept as far as it goes. `nc.model`, `nc.collection`, `nc.string`, `nc.oneOf`, `nc.int`, `nc.links`, `nc.reach`, `nc.view`, the `computed` and `views` places and `ncdb model build` keep their names and their meaning. Where this RFC differs:

1. **More field functions.** RFC 0001 shows four. This RFC adds `nc.float`, `nc.boolean`, `nc.set`, `nc.link`, `nc.ref` and the reserved `nc.richText` and `nc.stream`, because RFC 0004 defines floats, booleans, sets, single links and references into another space, and the agent task board needs a rich-text body. All are listed in [Public names](#public-names).
2. **Every field may be unset.** RFC 0001's `nc.int()` reads like a number that is always there. Under the additive-change rule it cannot be: a task written before `priority` existed has none. TypeScript types make every field optional.
3. **`priority: nc.int()` is last writer wins.** Two agents that raise a priority at the same time raise it once. RFC 0004 names the same limit; a counter is a different field type, possibly a plugin.
4. **`nc.view("tasks", ...)` inside `tasks` names its collection twice.** This RFC keeps RFC 0001's form as a placeholder and leaves the question to the computed-fields RFC: a view declared inside a collection may not need the collection's name, and a view across collections may belong at the model level.
5. **Personal fields: not any field.** RFC 0001 says any field can be marked as personal. This RFC allows it in v1 for basic values only, and only when the field is added, for the reasons in [Personal fields](#personal-fields).
6. **Indexes are declared.** RFC 0001 says queries run only through declared indexes, but its example declares none. This RFC adds the `indexes` place and `nc.index`.
7. **The schema module.** RFC 0001 does not say where the schema lives. This RFC has `ncdb model build` read `./model.ts` and its export `model` by default, and write `./model.lock.json` next to it.

## Agreement with GENESIS.md

- **"The schema is written once in TypeScript."** It is; the core and both SDKs read only the lock, which `ncdb model build` produces.
- **"Schema changes are additive; old clients keep data they do not understand yet."** The additive-change rule and [Data from a newer model](#data-from-a-newer-model).
- **"A check that refuses a model old clients cannot read."** The check runs twice: in `ncdb model build` and `ncdb model check`, and on every device when the app opens its database.
- **Principle 2, every copy converges.** The rule version of a field never changes without a migration (D12), so every client merges every field the same way. Two models in one space are the gap; D13 closes it.
- **Principle 4, say plainly what it does not do.** The limits of parked data, of deprecation and of personal fields are stated where they arise.
- **Principle 5, a developer does not need to care about keys.** The schema marks a field personal and names its person; the developer never sees a key.

A point for the owner, because a reader could infer more than the design delivers: GENESIS.md's phrase "a model old clients cannot read" is read here as "a model whose data old clients cannot keep". Old clients do not read added fields, unknown options or new link targets; they keep them, report them as parked, and read them after an upgrade. The accepted changes in [Accepted](#accepted) rely on this reading. Accepted (Carsten, 2026-11-08).

## Public names

Every public name this RFC introduces or keeps, for the owner to approve in one place. "RFC 0001" means the name comes from RFC 0001's example; "new" means this RFC introduces it; "reserved" means the name is taken now and designed later.

**Schema functions** (from `@nocentraldb/sdk`, under the namespace `nc`):

| Name | What it is | Origin |
| --- | --- | --- |
| `nc` | The namespace export of `@nocentraldb/sdk` that holds the schema functions | RFC 0001 |
| `nc.model` | Declares the model: the collections by name | RFC 0001 |
| `nc.collection` | Declares a collection: its fields, and the places `computed` and `views` | RFC 0001 |
| `nc.string` | A string, basic value | RFC 0001 |
| `nc.int` | A signed 64-bit integer, basic value | RFC 0001 |
| `nc.float` | A binary64 number, basic value | new |
| `nc.boolean` | A boolean, basic value | new |
| `nc.oneOf` | A choice from a fixed list of options, basic value | RFC 0001 |
| `nc.set` | A set of basic values | new |
| `nc.link` | A link to one object | new |
| `nc.links` | Links to many objects | RFC 0001 |
| `nc.ref` | A reference to one object, which may live in another space | new |
| `nc.richText` | Rich text | reserved |
| `nc.stream` | A stream | reserved |
| `nc.reach` | A computed function | RFC 0001, reserved |
| `nc.view` | A view | RFC 0001, reserved |
| `nc.Infer` | The TypeScript type of an object of a collection, as the app reads it | new |
| `.personal(` | Modifier: marks a field personal, with the option `person` (`"self"` or a link field's name) | new |
| `.describe(` | Modifier: a description, copied into the lock | new |
| `.deprecated(` | Modifier: keeps a field or a collection in the model and the data, hides it from the app | new |
| `computed`, `views` | Two places in `nc.collection`'s second argument | RFC 0001 |
| `indexes` | The place for indexes in `nc.collection`'s second argument | new |
| `nc.index` | Declares an index: fields in order, with the options `equal` and `scope` (`"space"`, [RFC 0009](0009-sdk-api.md#queries-and-indexes)) and, per field, `field` and `direction` (`"asc"`, `"desc"`) | new |

**CLI** (`ncdb`):

| Name | What it does | Origin |
| --- | --- | --- |
| `ncdb model build` | Reads the schema and the previous lock, checks the change, writes `model.lock.json` | RFC 0001 |
| `ncdb model check` | The same check, writes nothing, fails if the lock is out of date or the change is refused | new |
| `--model`, `--lock` | The paths of the schema module and the lock; defaults `./model.ts` and `./model.lock.json` | new |

**`model.lock.json`** (an on-disk format, also reserved for the owner):

| Name | Origin |
| --- | --- |
| The file name `model.lock.json` | RFC 0001 |
| Top-level keys `format`, `model` (`id`, `version`, `hash`), `plugins`, `collections` | new |
| Collection keys `since`, `fields`, `computed`, `views`, `indexes`, `deprecated`, `deprecatedReason` | new |
| Index keys `fields` (`field`, `direction`), `equal`, `scope`, `since` | new |
| Field keys `type`, `rule`, `since`, `options`, `of`, `targets`, `personal` (`person`), `description`, `deprecated`, `deprecatedReason`, `config` | new |
| Plugin keys `npm`, `crate` | new |
| The type id namespace `ncdb/` and the ids `ncdb/string`, `ncdb/int`, `ncdb/float`, `ncdb/boolean`, `ncdb/choice`, `ncdb/set`, `ncdb/link`, `ncdb/links`, `ncdb/ref` | new |

## Rejected options

| Option | Why not |
| --- | --- |
| Reuse a validation library's schema (Zod, Valibot) | They describe the shape of a value, not how it merges, and their types do not map to field types. Wrapping them would still need every concept of this RFC, and tie the public API to another project's releases. |
| A schema language of our own, in its own file (as Prisma does) | A second language to learn, parse and tool. GENESIS.md says the schema is written in TypeScript, and a developer's editor already understands TypeScript. |
| Table builders in the style of Drizzle | They describe SQL columns and migrations. Our local view is SQLite, but the developer's model is objects with merging fields, and migrations are what the additive-change rule avoids. |
| Classes with decorators | Decorators describe one object at a time and need a compiler setting; a model is easier to read, diff and infer types from as one value. |
| The SDKs read the TypeScript, or code generated from it, instead of a lock | The Rust core cannot read TypeScript, and generated code has no history. The lock is a small, versioned contract that every runtime reads the same way. Typed code can be generated from the lock later. |
| A lock without history, only the current model | The build could not tell an added field from a renamed one, so it could not refuse a non-additive change. `since` and deprecated entries are the history the check needs. |
| Numeric field ids in the lock | RFC 0002 names fields in changes by name (its decision D5). Numbers would need a registry that two branches could fill differently. |
| YAML or CBOR for the lock | YAML parses differently in different libraries; CBOR is not readable in a pull request. JSON is read the same everywhere and reviewed like code. |
| Semantic versioning of the model | Every accepted change is additive, so a model never has a breaking version. A whole number that grows by one says all there is. |
| Required fields and default values | See [No required fields and no defaults](#no-required-fields-and-no-defaults). |
| Removing a field after it has been deprecated for some versions | An old client can return after any time and still write the field. There is no version after which no client will. |

## Open questions

None. The owner has decided every question this RFC asked.

## Decisions

Each decision this RFC asked of the owner, in short form: what was chosen, by whom and when, why, and the options not taken. The design above describes the chosen options.

### D1. The style of the schema

Decided (Carsten, 2026-11-08): functions under one namespace, `nc.*`, with chained modifiers, and the model as one exported value.

Why: it is what RFC 0001 shows, needs no compiler setting, and lets TypeScript infer the app's types from the model.

Not taken:

- Classes with decorators.
- A schema file in a language of its own.

### D2. The public names

Decided (Carsten, 2026-11-08): the names in [Public names](#public-names), as listed.

Why: they are the first thing a developer types; deciding them here let the SDK and `ncdb` be built against them. The names of reads (conflicts, lost removals, link states) went to the SDK's API review, [RFC 0009](0009-sdk-api.md).

Not taken:

- Approve RFC 0001's names only, and decide the new ones in the SDK's API review.

### D3. The format of the lock

Decided (Carsten, 2026-11-08): JSON, in one canonical form, committed next to the schema.

Why: a reviewer reads the change to the model in the pull request, and every runtime parses it the same way.

Not taken:

- CBOR, like the changes.
- YAML.

### D4. Model versions

Decided (Carsten, 2026-11-08): a whole number per change, a content hash, a lineage id, and `since` per collection and field.

Why: the number is what a person and an error message can use ("needs model version 7"), the hash says whether two locks describe the same model, the lineage id catches a lock that was started over, and `since` lets an old client say which version it is missing.

Not taken:

- A content hash only.
- Semantic versions.

### D5. Which changes count as additive

Decided (Carsten, 2026-11-08): the changes in [Accepted](#accepted), among them appending options and adding link targets; old clients keep the data and read it as an unknown option or a wrong collection.

Why: a task board without a way to add a status is not usable for long, and old clients lose nothing. It relies on the reading of GENESIS.md in [Agreement with GENESIS.md](#agreement-with-genesismd).

Not taken:

- Only new collections, new fields and descriptions; options and link targets fixed once added.

### D6. Required fields and defaults

Decided (Carsten, 2026-11-08): none in v1; every field may be unset.

Why: an object written under an older model lacks newer fields, so "unset" exists anyway. Defaults or required fields would pretend otherwise, and required fields cannot be enforced on data from other members.

Not taken:

- Defaults, recorded in the lock and never changed once set.
- Required fields that the SDK enforces on write.

### D7. Removing a field from the app

Decided (Carsten, 2026-11-08): `.deprecated()` keeps the field in the lock and the data forever and hides it from the app; it cannot be undone.

Why: no version guarantees that no old client will write the field again.

Not taken:

- Removing a deprecated field from the lock after a number of versions.

### D8. Renaming

Decided (Carsten, 2026-11-08): no renames in v1; the TypeScript key is the stored name.

Why: v1 is simpler without them. An alias is additive and can come later as an optional key, without a format change.

Not taken:

- An alias, so the TypeScript name can differ from the stored name.

### D9. Personal fields

Decided (Carsten, 2026-11-08): `.personal({ person })` with `"self"` or a link field; basic values only; only when the field is added.

Why: personal sets and links would break set identity and link resolution (see [Personal fields](#personal-fields)).

Not taken:

- As chosen, but also on sets and links.
- The person as any field's value, such as an email address, instead of an object: it ties the person key to a value that can change and is itself personal.

### D10. The model version in changes

Decided (Carsten, 2026-11-08): the writer's model version goes into the payload as a "may ignore" key, so an old client can report which model version it needs to read its parked changes. [RFC 0002, "The writer's model"](0002-change-and-log-format.md#the-writers-model) puts it in payload key 64, together with the writer's `model.id` (D13).

Why: it costs a few bytes inside the ciphertext, the relay cannot see it, and old clients can ignore it.

Not taken:

- No model version in changes; the SDK reports only that changes are parked, and why.

### D11. The check on the device

Decided (Carsten, 2026-11-08): the SDK runs the additive check against the lock it last opened the database with, and refuses to open on a refused change.

Why: the build can be bypassed by deleting the lock or by a bad merge; the device holds the data that would be at risk.

Not taken:

- Trust the build check alone.

### D12. Moving a field to a new rule version

Decided (Carsten, 2026-11-08): the build refuses it until a migrations RFC defines how every member, offline ones included, moves over. A new field can use the new rule version at once, and the old field can be deprecated.

Why: RFC 0004 allows a new rule version only with a migration that every member runs first. A member that is offline for weeks cannot be shown to run the new rule, so this keeps RFC 0004's rule without a large design v1 can avoid.

Not taken:

- Write the migrations RFC before v1, and allow it from the start.
- Allow it once a stated share of members runs the new version, without a migration: it diverges for every member it misses.

### D13. Two models in one space

Decided (Carsten, 2026-11-08): both of two guards. A space records the `model.id` it was created with in its first control entry, and the SDK refuses to write into a space of another model ([RFC 0008, "Genesis"](0008-membership.md#genesis)). Each change carries its writer's `model.id` in payload key 64, with the model version of D10, and a member parks every change of another model, whole ([RFC 0002, "The writer's model"](0002-change-and-log-format.md#the-writers-model)).

Why: the first guard stops honest apps at the first write; the second makes a buggy or hostile client's changes park the same way on every member instead of folding by the wrong rule. Two apps that should share a space then share one model lineage.

Not taken:

- The space's `model.id` only.
- The `model.id` in each change only.
- Allow it and document the risk.

### D14. One person across spaces

Decided (Carsten, 2026-11-08): a person id, 128 random bits separate from object ids, that the core keeps on every object a personal field names as its person, and carries over when the app records the same person in another space. Each member maps a person id to the person's person-key id in each space, so a shred reaches every space the member can map. [RFC 0003, "The person key"](0003-keys-and-cryptography.md#the-person-key) keeps the person id on members only, inside the encrypted payload; the relay sees only per-space person-key ids. [RFC 0009, "Personal fields"](0009-sdk-api.md#personal-fields) gives the app the option `samePersonAs`.

Why: one erasure reaches every space without the app finding each copy, which is the promise of RFC 0001's team lead.

Not taken:

- Per space only: an erasure work order lists each person object, space by space, which puts the search back on the app.
- A person id derived from an external identifier, such as a hash of an email address: it is guessable and is itself personal data.

### D15. Personal data in free text

Decided (Carsten, 2026-11-08): it is the app's job. An app knows which free-text fields may name persons, stores a reference to the person instead of the name, and resolves it at display time; NoCentralDB says plainly that it does not detect names (see [Personal data in free text](#personal-data-in-free-text)).

Why: scanning text for names would be a guess that misses names and lets readers believe erasure reached them (principle 4).

Not taken:

- The core scans free text for names of known persons and seals them.
- Say nothing beyond "erasure does not reach unmarked text", which leaves every app to find the risk on its own.

### D16. nc.int in TypeScript

Decided (Carsten, 2026-11-09): `nc.Infer` types an `nc.int` field as a `number`. A write must be a safe integer, or the SDK refuses it with `invalidValue`; a stored value beyond that range reads `undefined`, is named in `$raw`, and `space.inspect` returns it as a `bigint` (see [Types for the app](#types-for-the-app)).

Why: building `ncdb model build` (NOC-70) found that `nc.int` is a signed 64-bit integer while a JavaScript `number` holds integers exactly only up to 2^53 − 1. Ints in apps are priorities, counts, sizes and positions, far below that, and RFC 0009's examples write them as `number`. Values beyond the range show up in `$raw`, never silently rounded (principle 4). A variant typed `bigint` can come later as a new field function, with the same field type `ncdb/int`.

Not taken:

- `bigint`: exact, but every read and write uses `3n`, `JSON.stringify` throws on it, and mixing it with a `number` throws.
- `number | bigint`: exact, but every use of every int field has to handle both types.

### D17. Deprecating a collection

Decided (Carsten, 2026-11-09): `.deprecated(reason)` chains on `nc.collection(...)` as on a field, with the same rule that it cannot be undone (D7). The lock gives the collection `deprecated`, the model version that deprecated it, and `deprecatedReason`. The SDK hides the collection from its types and refuses writes into it with `deprecated`.

Why: building `ncdb model build` (NOC-70) found that the lock had a collection key `deprecated` but the schema had no way to write it. One name for one idea: a developer who knows `.deprecated()` on a field finds it on a collection, and the lock reads the same in both places.

Not taken:

- An option in `nc.collection`'s second argument, `{ deprecated: "reason" }`, which spells the same thing a second way.
- No deprecated collections in v1: the collection would stay in every type, and new objects could keep appearing in it.

### D18. How the schema declares a reference

Decided (Carsten, 2026-11-10): `nc.ref("collection")`, or a list of collections, as `nc.link` takes them. The lock writes `{ "type": "ncdb/ref", "rule": 1, "targets": [...] }`, with the field key `targets` that links use; adding a collection to `targets` is accepted and removing one is refused, as for links. The owner had decided the reference itself in NOC-29 (Carsten, 2026-11-09).

Why: a reference reads like a link in the schema and in the lock, and the one thing a developer has to learn is that the target may live in another space.

Not taken:

- One collection only: a field that may point to a customer or a lead would need two fields.
- No collection, `nc.ref()` to any object: the SDK could neither type the target nor check its collection.

### D19. How a reference is indexed

Decided (Carsten, 2026-11-10): one column per reference, the space id followed by the object id, 32 bytes, an SQLite `BLOB` compared by its bytes. The index holds the reference as stored, whatever its state; an unset reference, and a value that does not fit, index as unset ([RFC 0006, "Declared indexes"](0006-local-view.md#declared-indexes)).

Why: the index is the same on every member of the space, and "all orders of customer X" is one equality lookup, with the index shape RFC 0006 already has.

Not taken:

- Two columns, the space id and the object id: one field would become two columns, which no other type does.
- The object id only: an object id is unique only in its space, so a hostile member could make an order match a customer of another space.

### D20. How a read and a view show the three states

Decided (Carsten, 2026-11-10): `nc.Infer` types a reference as `{ space, id }`, and the state is not part of the read object. `space.inspect` gives it per reference as `status`, `"ok"`, `"notAccessible"` or `"missing"`, next to the reference's conflicts. `$conflicts` names the field for its conflicts only, never for its state. No view, index or computed field holds the state.

Why: a read object stays a function of its own space, as RFC 0009 has it, and the state appears where a link's status already does. A state is not a conflict, and it differs from member to member.

Not taken:

- The read object carries the state: two members with the same copy of the space would read different objects, and a view could not hold them.
- The SDK reads the target along with the reference: every read of an order reads a second space, and the read object depends on two spaces.

### D21. Whether a subscription follows a reference's state

Decided (Carsten, 2026-11-10): no. A subscription re-runs when the referencing object changes, not when the target arrives, changes or is deleted, and not when the member joins or leaves the target's space.

Why: RFC 0006's notifications carry the objects a fold touched, in the space it folded, and the app already holds the target's id to subscribe to it.

Not taken:

- Yes: the core would need a reverse map across spaces, and a fold in one space would notify the queries of another.

### D22. Modifiers, and many references

Decided (Carsten, 2026-11-10): a reference takes `.describe(` and `.deprecated(`. It cannot be `.personal(`, and cannot be the `person` of a personal field. There is no field of many references in v1.

Why: a personal field is sealed under its person's key in its own space, and a person in another space has no key there that a shred could destroy. NOC-29 decided a single reference; many references can come later as a field type of their own, without changing `ncdb/ref`.

Not taken:

- `nc.refs("collection")` as well, a set of references: a second type, and a field with many values cannot be indexed in v1.

## History

- **2026-11-08** · NOC-11 · The first version, with the decisions D1 to D15, which the owner decided as recommended (#35).
- **2026-11-09** · NOC-54 · The writer's `model.id` and model version travel in payload key 64, as RFC 0002 puts them for D10 and D13; parked changes of another model are counted apart (#70).
- **2026-11-09** · NOC-55 · How to give feedback on this RFC (#72).
- **2026-11-09** · NOC-35 · "Personal fields" quotes RFC 0001's wording: only basic values can be personal (#76).
- **2026-11-09** · NOC-75 · Follows RFC 0009: the index option `scope: "space"` and its lock key `scope`, `nc.Unknown` for an unknown option, and the refusal `noPerson` (#87).
- **2026-11-09** · NOC-73 · The example lock in canonical key order; D16 (`nc.int` as a `number`) and D17 (deprecating a collection), decided as recommended (#88).
- **2026-11-10** · NOC-63 · `nc.ref`, a reference into another space, with the decisions D18 to D22, decided as recommended (#92).
- **2026-11-10** · NOC-101 · Rewritten as one complete text: every amendment folded into the design, and the decisions in short form (#104).

## How this RFC is tested

`scripts/rfc-0005.test.mjs` and `scripts/rfc-0005-integrated.test.mjs`, run by `repo:test` in `pnpm gates`, check that this file links to RFC 0001's "The design in one page", that every link and its anchor resolve, links from other files included, that the sections it needs are present, that every `nc.*` function, modifier and `ncdb model` command used in an example is listed under [Public names](#public-names), that every JSON example parses, that the example lock is byte for byte the file `ncdb model build` writes (`crates/ncdb-core/tests/model_lock/rfc-0005-v2.lock.json`), that each rule of the design stands where it belongs, and that each decision records the owner's choice. The design itself is tested where it is built: the Rust tests of `ncdb model build` and `ncdb model check` with a fixture for each accepted and each refused change.
