# RFC 0018: The model API for an app that decides placement: named indexes, one reference type, edges, indexes through a reference

REQUEST FOR COMMENTS · NOVEMBER 2026 · Ticket NOC-551

This RFC lets an app model its data the same way whichever space each object lives in. An app such as nexflow.it uses a space only to say who sees something; what an object belongs to is a field or an edge of the model, and the app decides which space each object goes into. Its relations then cross spaces routinely: a person's placement of a shared project into one of the person's own areas is an edge in the person's private space, while the project lives in a shared one. Today the model reads differently with each cut of the spaces: a relation is an `nc.link` inside a space and an `nc.ref` across spaces, a many-to-many relation is a collection the app builds by hand, an index cannot sort by a field of the object a reference names, and an index spells its equality prefix as a count ([The problem](#the-problem)).

It builds on the schema of [RFC 0005](0005-schema-and-model-lock.md), the links and references of [RFC 0004](0004-merge-rules-basic-types.md) and the indexes of [RFC 0006](0006-local-view.md); [How this RFC relates to the others](#how-this-rfc-relates-to-the-others) names the rest.

**How to give feedback.** Start a discussion in the category [RFC feedback](https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback), naming RFC 0018 and the section you mean, or open a pull request against this file to change its text. [Giving feedback](README.md#giving-feedback) says how, and what happens to your point.

**Status.** Draft. The owner decides it in its pull request. Each choice that decided text does not settle is a numbered question in a ticket for the owner, which the pull request names, with its options; this text is written with the first option of each, and is rewritten where an answer differs. Each requirement is decided and built on its own, and R3 may come first: its section depends on no other.

**At 0.1.** Not built. Release 0.1 has today's index form (`nc.index(fields, { equal })` and `.equal(...)`), `nc.link` and `nc.ref` as two field types, no edge collection and no index through a reference.

**Out of scope.** Computed fields and views, which a later RFC designs; this RFC says only what they may follow. Many references in one field (`nc.refs`). Edges with more than two ends. A range query, whose place in an index [R3](#r3-named-equality-and-order-in-an-index) names but does not add. Moving a whole space.

## Summary

| Requirement | Answer |
| --- | --- |
| R3. Named equality and order | `nc.index({ where: ["status"], orderBy: ["order"] })`, with `"-field"` for a descending order, and `.where(...)` in a query. It writes the same lock as today's form, byte for byte, so no lock, cursor or model hash changes, and today's form stays valid. |
| R4. One reference type | `nc.ref` names a target in its own space or in another. A reference into its own space resolves as a link does, so nothing a link gives is lost. A field locked as `ncdb/link` keeps its type and reads as a reference, with no data or lock migration. A move writes references among the moved objects with the new space. |
| R1. `nc.edge` | A collection with two reference ends, optional fields and one index per end. Either end may live in another space. An edge is unique per pair: its id derives from its ends, so creating it again changes nothing. An end that is deleted, missing or moved leaves the edge as written. |
| R2. An index through a reference | `nc.index({ where: ["area", "project.state"], orderBy: ["project.priority"] })` sorts an edge by fields of the object an end names. The core keeps the target's fields beside the edge in the view and updates them when the target changes, in any space. A target that does not resolve leaves the index. Such an index is member-local, as a reference's state is. |
| What the relay sees | Nothing new: every value is in sealed payloads, and every index is local. |
| What it costs a device | Measured: about 0.1 ms more to fold a change to an object two edges name, 24 ms for one that 10,000 edges name; a page of 100 out of thousands in 0.2 ms ([What it costs](#what-it-costs)). |

## The problem

nexflow.it places objects by what the person sees, and models them by what they are. One person keeps areas of life in a private space. A project lives in a space shared with its team. The person files the project under one of their areas. That filing is the person's own: nobody else in the project's team may see it or change it. So it lives in the person's private space, and it names the project in the shared space.

The model wants three collections, `areas`, `projects` and `projectAreas`, and one query: the active projects of one area, about 100 out of thousands, sorted by the project's priority, a page at a time. With the design up to RFC 0017, an app:

1. writes `projectAreas` as a collection with two `nc.ref` fields, and an index per field, by hand;
2. cannot stop the person's devices from filing one project under one area twice;
3. cannot sort or filter the filings by the project's state or priority: an index holds only the fields of its own collection, and a reference's target lives in another space. So the app reads every filing of the area, then every project, and filters and sorts in memory, which reads rows it throws away ([GENESIS.md, principle 6](../../factory/GENESIS.md#principles-in-priority-order));
4. cannot copy the project's state onto the filing instead, because another member of the project's space changes the state and cannot write the person's private space;
5. writes `nc.link` where it happens to keep two objects in one space and `nc.ref` where it does not, so moving an object to another space ([RFC 0016](0016-moving-an-object.md)) changes which field type its relations should have had;
6. writes `nc.index(["status", "priority"], { equal: 1 })`, where the count says which fields are fixed by equality.

## Terms

| Term | Meaning in this RFC |
| --- | --- |
| Space, member, change, object, collection, field | As in RFC 0002 and RFC 0005. |
| Link, reference | As in RFC 0004: `nc.link` (`ncdb/link`) points into its own space by object id; `nc.ref` (`ncdb/ref`) names a space id and an object id. |
| In-space reference | A reference whose space id is the space of the object that holds it. |
| Cross-space reference | A reference whose space id names another space. |
| Edge | An object of an edge collection: two ends and optional fields. |
| End | One of an edge's two reference fields. |
| Through field | A field of the object a reference names, used in an index of the referencing collection, written `reference.field`. |
| Through index | An index with at least one through field. |
| Equality prefix | The leading fields of an index that a query fixes with equality (RFC 0006). |

## R3. Named equality and order in an index

This section stands on its own: it needs nothing from R4, R1 or R2, and is decided and built first.

### The form

An index names the fields a query fixes with equality, `where`, and the fields that order the result, `orderBy`:

```ts
indexes: {
  byStatusOrder: nc.index({ where: ["status"], orderBy: ["order"] }),
  latest: nc.index({ orderBy: ["-occurredOn"] }),
  openInProject: nc.index({ where: ["status"], orderBy: ["priority"], scope: "space" }),
}
```

- **`where`** lists the equality prefix in index order. Each name is a field of the collection that can be indexed ([RFC 0005, "Indexes"](0005-schema-and-model-lock.md#indexes)). It may be empty or left out.
- **`orderBy`** lists the fields after the prefix, in order. A name sorts ascending; a name with a leading `-` sorts descending. A field name cannot start with `-` ([RFC 0005, "Names"](0005-schema-and-model-lock.md#names)), so the sign is never part of a name. It may be empty or left out; the order is then the space id and object id every index ends with ([RFC 0006, "Declared indexes"](0006-local-view.md#declared-indexes)).
- **`scope: "space"`** stays as it is.
- At least one of `where` and `orderBy` names a field, and no field is named twice. The build refuses anything else, as it refuses an index on an unknown field today.

A query fixes the prefix with `.where(...)`, one value per `where` field, in that order, which TypeScript checks for count and type as it checks `.equal(...)` today:

```ts
const page = await db.find(model.tasks.indexes.byStatusOrder.where("open").take(50));
```

### The lock does not change

The named form is a second spelling of the same index. `ncdb model build` writes it as RFC 0005 writes today's form: `fields` is the `where` fields, each ascending, followed by the `orderBy` fields with their directions, and `equal` is the number of `where` fields.

```ts
nc.index({ where: ["status"], orderBy: ["priority"] })
nc.index(["status", "priority"], { equal: 1 })            // the same lock entry
nc.index({ orderBy: ["-occurredOn"] })
nc.index([{ field: "occurredOn", direction: "desc" }])    // the same lock entry
```

So a model that moves to the named form builds the same `model.lock.json` byte for byte, keeps its model hash, and every cursor stays valid ([RFC 0006, "Ordered pages"](0006-local-view.md#ordered-pages)). Every existing lock reads as it does, the core needs no change, and an app switches index by index. An index whose equality fields are declared descending in today's form, which changes no result since the query fixes them, builds a different lock entry in the named form; the build reports it as a changed index, which is accepted, and its old cursors are refused, as for any changed index ([RFC 0005, "Accepted"](0005-schema-and-model-lock.md#accepted)).

Today's form stays valid, and `.equal(...)` stays as the same call as `.where(...)`. The documentation and the examples use the named form.

In Rust, which declares no schema and queries through the lock, the query names the call `.where_eq(values)`, since `where` is a keyword; `.equal(values)` stays.

### Where a range query goes

No release has a range query: a query fixes the equality prefix and pages ([RFC 0009, "Queries and indexes"](0009-sdk-api.md#queries-and-indexes)). When one comes, it bounds the first `orderBy` field. That is the one field after the prefix where a range stays a single seek and walk of the index: RFC 0006 already bounds that key to seek to a cursor ([RFC 0006, "Declared indexes"](0006-local-view.md#declared-indexes)). A range on a later `orderBy` field would walk every row of the earlier ones, which principle 6 rules out. The named form shows where the range applies without counting: after the `where` fields, on the first `orderBy` field.

### Options not taken

- **Replacing today's form.** One spelling only, but every model and every example would have to change at once, and a model that keeps the old form would be refused for no change in what it means.
- **A new lock shape with `where` and `orderBy` keys.** The lock would mirror the schema, but every core and every lock reader would need a new format, old locks would need a conversion, and the model hash of every app would change, which refuses every cursor.
- **`{ field, direction }` objects in `orderBy`.** No sign to learn, but the common case would read as long as today's form. Today's form keeps the objects for an app that wants them.
- **Directions on `where` fields.** A query fixes them, so their direction changes no result.

## R4. One reference type

### What a link gives today that a reference does not

RFC 0004 and RFC 0016 decide links and references apart. The differences, from the decided text and the core:

| | `nc.link` (`ncdb/link`) | `nc.ref` (`ncdb/ref`) |
| --- | --- | --- |
| Where the target lives | In the object's own space; the SDK refuses another ([RFC 0004, "Links"](0004-merge-rules-basic-types.md#links)) | In any space, its own included ([RFC 0004, "The value and its encoding"](0004-merge-rules-basic-types.md#the-value-and-its-encoding)) |
| Value | The object id, 17 bytes of CBOR | `[space id, object id]`, 35 bytes |
| Index column | 16 bytes | 32 bytes, the space id first |
| How it resolves | From the space's copy alone: **ok**, **deleted**, **missing**, **wrong collection**, **moved** | From the member's copies: **ok**, **not accessible**, **missing**; a deleted target and one in a collection the field does not name both read **missing**; a trace is followed, up to 8 ([RFC 0004, "How a reference resolves"](0004-merge-rules-basic-types.md#how-a-reference-resolves)) |
| Same on every member | Yes: a function of the space's changes, so it is shared state | No: it depends on which spaces the member holds, so it is local information |
| Followed by computed fields and views | Yes, once the computed-fields RFC designs them (`nc.reach`) | Never |
| The `person` of a personal field | Yes (`person: "assignee"`) | No ([RFC 0005, "References into another space"](0005-schema-and-model-lock.md#references-into-another-space)) |
| Many targets in one field | `nc.links`, a set | None in v1 |
| Read cost | One row of the same copy | A lookup of the named space's copy, then one row, and one more per trace |
| In a move | Kept among objects moved together; a link to an object left behind is cleared in the new space and counted in `clearedLinks` ([RFC 0016, "Links in the moved object"](0016-moving-an-object.md#links-in-the-moved-object)) | Kept as written, naming the space it names |
| A link or reference to the moved object | Reads **moved**, with the new space | Follows the trace for a member that holds the old space; **not accessible** for one that does not |

So a reference into its own space loses three things a link has: the statuses **deleted** and **wrong collection**, being followed by computed fields, and naming a person. These all hold for a link only because its target lives in its own space, where every member holds it. A reference whose space id is its own space has the same property.

### One function, two cases

`nc.ref` is the one function for a reference to one object, wherever the target lives:

```ts
projects: nc.collection({
  title: nc.string(),
  owner: nc.ref("people"),        // a person in this space or another
  area: nc.ref("areas"),
}),
```

The SDK sees at write where the target lives. A reference field takes `{ space, id }`, an object the SDK read, which carries `$space` and `$id`, or a bare object id, which names the written object's own space. The value is stored as `ncdb/ref` stores it, `[space id, object id]`, in every case.

**An in-space reference resolves as a link does.** When the space id of the value is the space of the object that holds it, the SDK resolves it from that space's copy alone, with a link's statuses: **ok**, **deleted**, **missing**, **wrong collection** and **moved** ([RFC 0004, "Resolution"](0004-merge-rules-basic-types.md#resolution)). Whether a reference is in-space is a function of its value and its object's space, which every member of the space reads the same, so the status is shared state as a link's is, and the computed-fields RFC may follow an in-space reference as it follows a link. A cross-space reference keeps the three states of RFC 0004, member-local, and nothing computed follows it.

**A reference that names a person is locked as a link.** A field that is the `person` of a personal field must point into its own space, since a person key lives in one space ([RFC 0003, "The person key"](0003-keys-and-cryptography.md#the-person-key)). `ncdb model build` locks such a field as `ncdb/link`, and the SDK refuses a target in another space for it with `invalidValue`. So no core needs a person resolved through a reference, and a hostile member cannot name a person in another space: a link value cannot.

**Many targets stay `nc.links`.** A set of links in one space keeps its type and its rule. Many targets across spaces are edges ([R1](#r1-ncedge)), which also carry fields of their own and can be indexed, which a field of many values cannot.

### Existing link fields read as references

A field locked as `ncdb/link` keeps its type. A change of field type is refused, because two members would merge one field by two rules ([RFC 0005, "Refused"](0005-schema-and-model-lock.md#refused)), and no data or lock needs to change:

- **The build accepts `nc.ref` over a field locked as `ncdb/link`,** and keeps `ncdb/link` with its targets, rule version and `since`. The lock and the model hash do not change. `nc.link` and `nc.links` stay valid as they are.
- **The SDK reads such a field in a reference's form,** `{ space, id }` with the object's own space, and resolves it as an in-space reference, which is how it resolves today.
- **A write of a target in another space to such a field** is refused with `invalidValue`, naming the field: a link value cannot hold a space. An app that wants that field to reach across spaces adds a new `nc.ref` field and deprecates the old one ([RFC 0005, "Modifiers"](0005-schema-and-model-lock.md#modifiers)); a value it copies over is a write the app makes.

The read shape changes for the app's code, from an id to `{ space, id }`, when the developer writes `nc.ref` for such a field. It is a change in the app's source, which the type checker shows; nothing changes for another member or for stored data.

### When the source or the target moves

A move ([RFC 0016](0016-moving-an-object.md)) writes the moved objects' state as a create in the new space. A reference keeps working whether source and target end up in one space or not:

- **Among objects moved together**, the move writes each reference whose target moves with it with the new space. In-space references stay in-space, as links among them stay valid today.
- **To an object left behind**, a reference in a moved object keeps naming the old space. It becomes a cross-space reference and is not cleared, unlike a link. A field locked as `ncdb/link` keeps RFC 0016's rule: it cannot name the old space, so it is cleared and counted in `clearedLinks`.
- **To the moved object, from an object that did not move,** a reference resolves through the trace as RFC 0004 decides: **moved** with the new space when it is in-space, and the state where the object is now when it is cross-space, for a member that holds the old space. A member that holds only the new space reads such a reference as **not accessible** until a member of the referencing space writes it with the new space. [R1, "An end that moves"](#an-end-that-moves) says how the SDK names those references.

### A reference to an object the reader cannot see

A reference into a space that only some members hold is the normal case, not a fault. An app that keeps notes in a person's private space and moves a project with its actions into a shared space leaves each action's reference to its note naming the private space: the move writes it as it stands, where a link would be cleared ([When the source or the target moves](#when-the-source-or-the-target-moves)). For the person, the reference reads **ok**. For every other member of the shared space it reads **not accessible**: their database holds no copy of the private space ([RFC 0004, "How a reference resolves"](0004-merge-rules-basic-types.md#how-a-reference-resolves)).

- **What it reveals.** Not accessible says only that this member holds no copy of the named space. It does not say whether the note exists, was deleted or was never written, and it shows none of the note's fields. The members of the shared space read the reference's value, the private space's random id and the note's random object id, as RFC 0004 says members of a referencing space do ([RFC 0004, "What the relay and the members see"](0004-merge-rules-basic-types.md#what-the-relay-and-the-members-see)): that the action points at something in a space they are not in, and nothing more. An app that must not show even that keeps the reference out of the shared space, for example in an edge in the private space ([R1](#r1-ncedge)).
- **In a find.** An index on the reference itself holds the action by the reference's value, whatever its state, so every member of the shared space finds the same actions in it ([RFC 0006, "Declared indexes"](0006-local-view.md#declared-indexes)). An index through the reference, such as actions ordered by `note.title`, holds only the rows whose note this member can read ([R2, "Which rows it holds"](#which-rows-it-holds)), so the other members do not find the action in it at all. An app lists the actions of a project through an index on the action's own fields, and uses an index through the note only for a view that is about the notes.
- **On read.** The action reads as it does for every member: the reference as `{ space, id }`, with its state in `space.inspect`, never in the read object ([RFC 0009, "Get by id"](0009-sdk-api.md#get-by-id)).

### Options not taken

- **A link that may name another space.** One type, but it changes what a shipped type's value means, and RFC 0004 refuses that for the reasons it gives ([RFC 0004, "References"](0004-merge-rules-basic-types.md#references)).
- **Converting link fields to references with a migration.** The lock would hold one type, but a field type change is refused, and a migration would write a change per object of every app that has a link field.
- **Every in-space reference keeps a reference's three states.** No new resolution rule, but an app that moves from `nc.link` to `nc.ref` would lose **deleted** and **wrong collection**, and computed fields could follow nothing in a model that uses only `nc.ref`.
- **Leaving references in moved objects as written.** No rewrite in the move, but a reference between two objects moved together would name the old space, so a member of the new space only would read it as **not accessible**, where a link between them reads **ok** today.

## R1. `nc.edge`

### The form

An edge collection is one building block: two ends, optional fields, and one index per end.

```ts
projectAreas: nc.edge({ project: "projects", area: "areas" }),
projectOrganizations: nc.edge(
  { project: "projects", organization: "organizations" },
  { role: nc.oneOf(["for", "partner"]) },
),
```

`nc.edge(ends, fields, options)` takes:

- **`ends`**: exactly two names, each with a collection or a list of collections, as `nc.ref` takes them. Each end is an `nc.ref` field of that name ([R4](#r4-one-reference-type)), so either end may live in the edge's own space or in another.
- **`fields`**: optional, as `nc.collection` takes them. A field may not take an end's name.
- **`options`**: optional, as `nc.collection`'s second argument: `indexes`, and `computed` and `views` once they are designed. `nc.edge(ends, {}, { indexes: { ... } })` adds indexes.

An edge collection is a collection: it has a name in the model, its objects live in a space, and it is read, written, moved and deleted as any other. The build adds one index per end, named `by` and the end's name with a capital first letter, `byProject` and `byArea`, each `nc.index({ where: [end] })`, so "every edge of this project" is one lookup:

```ts
const filings = await db.find(model.projectAreas.indexes.byArea.where(area).take(50));
```

An index name the app declares may not take one of these names. In the lock, an edge collection is a collection with the key `edge`, its two end names in order, beside its fields and indexes:

```json
"projectAreas": {
  "edge": ["project", "area"],
  "fields": {
    "area": { "rule": 1, "since": 3, "targets": ["areas"], "type": "ncdb/ref" },
    "project": { "rule": 1, "since": 3, "targets": ["projects"], "type": "ncdb/ref" }
  },
  "indexes": {
    "byArea": { "equal": 1, "fields": [{ "direction": "asc", "field": "area" }], "since": 3 },
    "byProject": { "equal": 1, "fields": [{ "direction": "asc", "field": "project" }], "since": 3 }
  },
  "since": 3
}
```

A lock reader ignores keys it does not know ([RFC 0005, "Names"](0005-schema-and-model-lock.md#names)), so a core that does not know `edge` reads the collection as a collection with two references, and folds it as it folds every other: only the writer's SDK needs `edge`. Adding an edge collection is adding a collection; turning an existing collection into an edge, or changing its ends, is refused, since its ids would no longer follow from its ends ([Unique per pair](#unique-per-pair)).

### Unique per pair

An edge's object id derives from its ends, so creating an edge that exists changes nothing. The id is the first 16 bytes of SHA-256 of the context string `"nocentraldb/edge"`, the collection's name, and each end's 32 bytes, the space id and the object id, in the order of `edge` in the lock. The account space derives its records' ids from what they record in the same way ([RFC 0010](0010-accounts.md)).

- **Creating it again** finds the same id. The SDK writes only the fields that differ from what its copy reads, and restores the edge if its copy holds it deleted, as an app restores any object ([RFC 0004, "Creating and deleting an object"](0004-merge-rules-basic-types.md#creating-and-deleting-an-object)).
- **Two devices that create it at once** write two creates of one id, which RFC 0004 already makes one object ([RFC 0004, "Two creates of one object id"](0004-merge-rules-basic-types.md#two-creates-of-one-object-id)); their fields merge by their types.
- **A delete and a create at once** are a delete and a restore of one object, a basic value: the later stamp wins, and the other stays readable as a conflict.
- **Its ends do not change.** The SDK refuses an update of an end with `notAllowed`: an edge to another target is another edge. A hostile member can still write another value into an end; the edge then reads the end as written, and its id no longer follows from its ends, which changes nothing for convergence, since the id is only where the edge is found.
- **The relay sees nothing of it.** The id sits in sealed payloads, as every object id does. A member of the edge's space who knows both ends can compute the id, and learns nothing it could not read in the space it holds.

The same pair in two spaces is two edges, as an object id is unique only in its space ([RFC 0002, "Object ids"](0002-change-and-log-format.md#object-ids)).

### An end that is deleted or missing

An edge stays as written whatever happens to its ends. An end resolves as a reference does ([R4](#r4-one-reference-type)): an in-space end reads **deleted** or **missing**, a cross-space end **missing** or **not accessible**. Nothing deletes the edge on its own: a member that deletes the project may hold no copy of the space with the edge, and a device that deleted edges on its own would write changes nobody asked for. The end's index, `byProject`, holds the edge by the value of its end, whatever the end's state, so it is the same on every member of the edge's space. An index through the end ([R2](#r2-an-index-through-a-reference-with-fields-of-the-referenced-object)) holds only edges whose end resolves, so "the active projects of this area" never lists a deleted project. When the project is restored, or arrives, the edge reads **ok** again with no write.

### An end that moves

When an end's target moves ([RFC 0016](0016-moving-an-object.md)), the edge stays as written, in its own space, and the end resolves through the trace ([When the source or the target moves](#when-the-source-or-the-target-moves)). A move takes objects of one space, so an edge in the project's own space moves with the project when the app includes it; an edge in another space, such as the person's private filing, stays where it is.

A member that holds the old space follows the trace, so nothing changes for it. A member that holds only the new space reads the end as **not accessible**. So the SDK lists, per space, the references in this member's copies whose target it reads through a trace, and the app or a person writes them with the new space:

```ts
const stale = await personal.references.throughTraces();  // [{ collection, id, field, now: { space, id } }]
await personal.references.follow(stale);                   // one write: each field gets the space it names now
```

`follow` writes each listed reference with the space its trace leads to, as one `write` in this space, and refuses a reference that is not on the list with `invalidValue`. The list is computed from the member's copies when the app asks, as RFC 0016 computes later edits. An edge to a moved end works without it for every member that holds the old space.

### Options not taken

- **Random ids, and the app finds duplicates.** No derived id, but two devices filing one project under one area offline would make two edges, and every reader would have to merge them.
- **Uniqueness checked by the index.** A unique index would need one place that sees every write before it is accepted, which NoCentralDB does not have ([RFC 0004, "A link to an object the member has not received"](0004-merge-rules-basic-types.md#a-link-to-an-object-the-member-has-not-received)).
- **Deleting an edge when an end is deleted (cascade).** It needs a member that holds both spaces to write a change nobody asked for, and a restore of the end would not bring the edge back.
- **The mover's devices write the edges in other spaces with the new space.** No stale references, but a device would write changes nobody asked for, in spaces the move does not name, as RFC 0016 declined for later edits.
- **Edges with more than two ends.** Not asked for, and each end adds an index.

## R2. An index through a reference, with fields of the referenced object

### The form

An index may name a field of the object a reference names, written `reference.field`, in `where` and in `orderBy`:

```ts
projectAreas: nc.edge({ project: "projects", area: "areas" }, {}, {
  indexes: {
    active: nc.index({ where: ["area", "project.state"], orderBy: ["-project.priority"] }),
  },
}),
```

```ts
const page = await db.find(model.projectAreas.indexes.active.where(area, "active").take(100));
page.items[0];                     // the edge: { $id, $space, project, area }
page.items[0].$targets.project;    // the project, as space.get reads it in its own space
```

- **The reference** is an `nc.ref` field of the collection, an edge's end or any other. The rule is the same for every collection; edges are where it is needed first.
- **The field** must be declared, with one field type, in every collection the reference names, and that type must be one an index takes ([RFC 0005, "Indexes"](0005-schema-and-model-lock.md#indexes)). One step only: `project.area.name` is refused.
- **The lock** writes the through field as its path, `{ "direction": "asc", "field": "project.state" }`. A field name never holds a dot, so a core that does not know through indexes refuses the lock with an unknown field, instead of reading the index on a field of its own collection.
- **A query** fixes a through field in `where` with a value of the target's field type, as for any field. Each item is the referencing object, and `$targets` holds, per reference the index reads through, the target as `space.get` reads it in its own space.

### Which rows it holds

A row is in a through index when the referencing object is not deleted and every reference the index reads through resolves to **ok** ([R4](#r4-one-reference-type)). A target that is missing, deleted, in a collection the field does not name, or in a space the member holds no copy of, takes the row out; a moved target is read where its traces lead, up to 8, as resolution reads it. The row comes back when the target resolves again, with no write.

The index holds the target's field values as this member's copies hold them. Two members that hold the same copies of the same spaces hold the same index. Two members of the edge's space that hold different spaces do not: a member who is not in the project's space finds no row for it. A through index is therefore member-local information, as a reference's state is ([RFC 0004, "How a reference resolves"](0004-merge-rules-basic-types.md#how-a-reference-resolves)), and principle 3 allows a member to show such information as local. It never enters a change, a space's shared state, a computed field or a view: those still never follow a cross-space reference. An index on an in-space reference only depends on the space's own copy, as a link does.

### How the view keeps it

RFC 0006 builds every index from the view, and gives step 4 of the fold, Derive, as the place where derived values are kept current inside the fold's transaction ([RFC 0006, "One change"](0006-local-view.md#one-change)). A through index is kept there:

- **Through columns.** For each through field, the referencing collection's table gets a column that holds the target's value, and one column for whether every reference the index reads through resolves. The SQLite index is built on those columns, as on any other, with the condition that the row resolves, as it leaves deleted objects out today ([RFC 0006, "Declared indexes"](0006-local-view.md#declared-indexes)). The columns are the view's, never in the log, and are rebuilt with it.
- **When the referencing object changes.** Folding a change to the edge, a create, a delete, a restore or an end, reads the target where it resolves and fills the columns.
- **When the target changes.** Folding a change that touches an indexed field of a target, its deletion, its collection or its move, in any space, finds the rows that reference it through an SQLite index on the reference's column, and fills their columns again. An edge's end index serves; for any other reference the core adds an index of its own. A trace leads the core from each space the traces name to the rows that reference the object there.
- **When the member joins or leaves a space.** A space's rows arrive by folds, so joining fills the columns as targets arrive. Leaving deletes the space's rows; the rows that reference that space are found by the 16-byte prefix of their reference column, and leave the index.
- **A shred** erases a person's fields in the target's space ([RFC 0006, "Control changes and changes that wait for them"](0006-local-view.md#control-changes-and-changes-that-wait-for-them)); a through column that held one is filled again in the same transaction, so an erased value leaves every index it was in.
- **A rebuild** fills every through column in one pass after the swap. Un-folding refolds a target's fields and runs Derive as any fold does.

The columns are a function of the rows of the view the member holds, filled from both sides, so the order in which edges and targets arrive does not change them: a target that arrives after its edge fills the edge's columns when it is folded, and an edge that arrives after its target reads it when it is folded.

**Subscriptions.** A fold that fills a row's through columns counts that row as touched ([RFC 0006, "One change"](0006-local-view.md#one-change), step 6), so a subscription on a through index re-runs when a project's priority changes in the project's space. This is the one place where a fold in one space calls the listeners of a query whose collection lives in another: the index reads the target, so the page changes with it. A subscription on any other query still follows no reference ([RFC 0009, "Subscribe"](0009-sdk-api.md#subscribe)).

### What it costs

A through index costs the device three things, which a few edges keep small and an object that many edges name makes large. Measured on SQLite 3.53 natively, with 10,000 projects and 30,000 edges, the derived values in a table of their own:

| What | Measured |
| --- | --- |
| A change to a project that 2 edges name, against no edges | 0.22 ms against 0.14 ms |
| A change to a project that 10,002 edges name | 24 ms |
| A page of 100 active projects of one area, with the projects | 0.18 ms; `SEARCH ... USING COVERING INDEX`, no sort |
| Storage per edge, the values and the index | 178 bytes |
| Filling every edge's values again after a rebuild | 122 ms for 30,000 edges |

- **The fold grows with the edges that name the changed object.** One write per such edge, inside the fold's transaction. An object named by 10,000 edges, such as a project every person files, adds about 24 ms to each change of it natively; in a browser it is estimated at two to three times that, and is measured when it is built. An object named by 100,000 edges would add about a quarter of a second natively.
- **The storage grows with the edges and the through fields.** One column and one index entry per edge and through field: under 200 bytes per edge for the example, measured with the values in a table of their own, which keeps its own copy of the ids; columns on the edge's own row take less.
- **A read costs its result.** The page is one walk of the index, plus one row of each target, read by its primary key.

These numbers are the measurements of a script against SQLite in Python on the build machine; the implementation measures them again, natively and in the browser, as RFC 0006's spike did.

### Options not taken

- **A join when the app reads.** No columns to keep, but "the active projects of one area, by priority" would read every edge of the area and every project, and sort in a temporary B-tree, which RFC 0006's plan check refuses and principle 6 rules out.
- **Copying the target's fields onto the edge as fields.** One index on the edge's own fields, but a member who changes the project cannot write the person's private space, so the copy would go stale.
- **Rows whose target does not resolve, sorted as unset.** More rows would be the same for every member of the edge's space, but the values of the rows that resolve still depend on the member's copies, and a query for "active" projects would read rows it skips.
- **Through indexes on in-space references only.** Every index would stay shared state, but the case that asks for it, a private filing of a shared project, is cross-space by design.

## What the relay sees

Nothing new. An edge is an object, its ends are references in a sealed payload, and its id is an object id in a sealed payload ([RFC 0002, "What the relay can see, and what it cannot"](0002-change-and-log-format.md#what-the-relay-can-see-and-what-it-cannot)). The named index form, the through columns and every index are built on the device from the view and never travel ([RFC 0005, "Indexes"](0005-schema-and-model-lock.md#indexes)). Rewriting references in a move happens inside the move's sealed create. Following references after a move is a `write` like any other.

## What this does not do

Stated plainly (principle 4):

- **A through index differs from member to member.** It holds only targets in spaces this member holds. Two members of one space can page the same query and see different rows.
- **A through index leaves out what the member cannot read.** An action whose note lives in a private space is in no index through the note for the other members of the action's space; an index on the action's own fields still holds it.
- **An object that many edges name makes its changes slower to fold.** The cost grows with the number of edges, on every device that holds them ([What it costs](#what-it-costs)).
- **Nothing keeps a target alive.** Deleting a project leaves every edge to it; the edges leave the through indexes, and stay in the end's index and in the space.
- **References to a moved object do not follow by themselves.** A member that does not hold the old space reads them as **not accessible** until a member writes them with the new space.
- **Uniqueness per pair holds for honest writers.** A hostile member can write another end into an edge, which then holds a pair its id does not match.
- **A field locked as `ncdb/link` stays in its space.** Writing `nc.ref` for it changes the app's code, not what the field can hold.
- **There is no range query yet.** R3 says only where one goes.

## Agreement with GENESIS.md

- **Principle 1, the relay never reads content.** Serves it: edges, their ids, references and the move's rewritten references are sealed; indexes are local.
- **Principle 2, every copy converges.** Serves it for shared state: edges merge by RFC 0004's rules, two creates of one edge are one object, and the status of an in-space reference is a function of the space's changes. A through index is member-local information, not shared state, and no computed field or view follows a cross-space reference, as RFC 0004 requires.
- **Principle 3, nothing is lost silently.** Serves it: an edge to a deleted target stays; a reference to an object left behind by a move stays instead of being cleared; references reached through traces are listed.
- **Principle 4, say plainly what it does not do.** [What this does not do](#what-this-does-not-do).
- **Principle 5, developers do not handle keys.** Unchanged.
- **Principle 6, apps read only computed results.** Serves it: "the active projects of one area, by priority" becomes a declared index whose read costs its result, instead of a read of every edge and every project.
- **Principle 7, one core.** Serves it: the through columns are kept by the core's fold, natively and in the browser alike.
- **Members only.** A through index reads only the copies this member holds, so it never shows more than `space.get` would; it gives no access to a space.
- **The cost of cross-space indexes on a device.** [What it costs](#what-it-costs) measures it, and what this does not do names the case that makes it large.

## Follow-up tickets

Each requirement becomes its own build ticket once decided, test first, with convergence tests in the harness where folds change:

- **R3 in both SDKs and the build:** the named form, `.where(...)` and `.where_eq(...)`, and the test that each named index builds today's lock entry byte for byte. RFC 0005 and RFC 0009 take the form.
- **R4:** in-space references resolved as links, the person rule, `nc.ref` over `ncdb/link` in the build, the write forms, and the move's rewrite of references among moved objects. RFC 0004, RFC 0005, RFC 0009 and RFC 0016 take it.
- **R1:** `nc.edge` in the build and both SDKs, derived ids, the end indexes, the refusal to update an end, and `space.references`.
- **R2:** through fields in the build and the lock, through columns kept in Derive, the plan check for through indexes, `$targets`, subscriptions, and the measurements natively and in the browser. RFC 0004, RFC 0005 and RFC 0006 take it.

## How this RFC is tested

The RFC tests in `scripts/` check this file's form: its status and what 0.1 builds, its links and the headings they name, its terms against the glossary, and its record. Each requirement's rules are tested when they are built, by the tickets above.

## How this RFC relates to the others

It adds an index form and through fields to [RFC 0005](0005-schema-and-model-lock.md)'s schema and lock, an edge collection to its collections, and changes how `nc.ref` and `nc.link` are written; it gives an in-space reference a link's resolution from [RFC 0004](0004-merge-rules-basic-types.md), and makes a through index member-local as RFC 0004 makes a reference's state; it keeps through columns in [RFC 0006](0006-local-view.md)'s Derive step; it adds `.where(...)`, `$targets` and `space.references` to [RFC 0009](0009-sdk-api.md); it rewrites references among moved objects in [RFC 0016](0016-moving-an-object.md)'s move; and it derives edge ids as [RFC 0010](0010-accounts.md) derives the account space's record ids.

## History

- 2026-11-27 · First published as a draft (NOC-551)
