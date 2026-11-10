# RFC 0001: NoCentralDB

REQUEST FOR COMMENTS · DRAFT 0.1 · OCTOBER 2026 · nocentraldb.io

A local-first database for people and agents.

**Changelog.**

- **2026-11-08** · NOC-44 · [Amendment 1: following RFC 0008](#amendment-1-following-rfc-0008) brings this RFC in line with the owner's decisions in [RFC 0008](0008-membership.md): groups are lists the SDK expands (withdrawn by the owner (Carsten, 2026-11-09) in [Amendment 5](#amendment-5-groups-leave-the-design)), the relay sees more of the control changes it checks, and an erasure destroys the key only once it is stable. The sentences it changes elsewhere are marked *(Amendment 1)*.
- **2026-11-09** · NOC-47 · [Amendment 2: following RFC 0007](#amendment-2-following-rfc-0007) links [RFC 0007](0007-relay.md)'s list of what the relay sees next to RFC 0002's, which no longer reads as complete on its own. It decides nothing new. The sentence it changes is marked *(Amendment 2)*.
- **2026-11-09** · NOC-34 · [Amendment 3: a browser copy takes its member key with it](#amendment-3-a-browser-copy-takes-its-member-key-with-it) says that a browser which deletes its copy deletes the member key too, and links the page [Browser apps and durability](../browser-durability.md). It decides nothing new. The sentence it changes is marked *(Amendment 3)*.
- **2026-11-09** · NOC-35 · [Amendment 4: following RFC 0005 on personal fields](#amendment-4-following-rfc-0005-on-personal-fields) says that only basic values can be personal, and only from the version that adds the field, names the app's duty for personal data in free text, and says that removing a member is not erasure, as [RFC 0005](0005-schema-and-model-lock.md) decided. It decides nothing new. The sentences it changes are marked *(Amendment 4)*.
- **2026-11-09** · NOC-76 · [Amendment 5: groups leave the design](#amendment-5-groups-leave-the-design) records the owner's decision to keep groups out of every definition for now. A member is a person's device, an agent or a sub-agent, and nothing else. The sentences it changes are marked *(Amendment 5)*.
- **2026-11-09** · NOC-75 · [Amendment 6: following RFC 0009](#amendment-6-following-rfc-0009) makes the example in "The design in one page" query a declared index, as [RFC 0009](0009-sdk-api.md) defines it, because views are not designed yet. It decides nothing. The sentences it changes are marked *(Amendment 6)*.

## For when a central database is the wrong place

Everyone works on their own copy, under their own keys. The relay in the middle cannot read any of it.

**Status.** NoCentralDB is not released yet. We have started building the core, and this RFC describes the design we are building toward. We want you to challenge it while changes are still cheap. The roadmap shows where we are.

Agents now read and write the same data as the people they work with, hundreds of records per task. Today almost every app routes each of those reads through a server, and that server can read everything it stores. NoCentralDB is a local-first database for teams of people and agents. Every member, human or agent, works on a full local copy of the spaces it belongs to, under its own keys. A relay that holds no keys carries the encrypted changes between the members.

## The problem

You are building an app where people and agents work on the same data: tickets, notes, customer records, a shared document. The usual setup has three problems.

- **Agents are slow on a network.** An agent that wants "the next 50 open tasks, with their blockers" makes dozens of calls, waits for each one, and hits rate limits. Most of its time goes into waiting for the network.
- **The server in the middle can read everything.** Row-level rights protect users from each other. They do not protect users from the server, its operators, its backups or the next breach. You have to explain that to your security review every time.
- **Erasure is a search.** When someone asks you to delete their data, you search the database, the backups, the logs, the search index and the copies your agents made.

For many apps a central database is still the right choice. If you need queries on the server, analytics across all users' data, or one global truth enforced at write time, keep it. This RFC is for the apps where a central database is the wrong place.

## Why now

Two things changed. First, agents became members of teams. They run on laptops, in sandboxes and on servers, and they need the same data their people use, at the speed of local memory. Second, devices can now hold the data and the keys. A browser can hold a non-exportable key that a passkey unlocks. A phone can store a team's complete working set. One core written in Rust runs natively and as WebAssembly in the browser.

So "trust the server" no longer has to be the default security model. Many apps need neither SQL on a server nor a central database. Your data can live where the work happens, on every device and with every agent you trust, under your own keys. Cloud services carry the encrypted changes but never hold the keys.

## The design in one page

- **Spaces.** A space is the unit of sharing, membership and keys. Every object belongs to exactly one space. Fine-grained rights come from many small spaces. A member is a person's device, an agent or a sub-agent. *(Amendment 5.)*
- **A full copy per member.** Each device and each agent keeps the complete, encrypted log of every space it belongs to, and folds it locally into an SQLite view. Reads never leave the device.
- **A relay that holds no keys.** The relay appends signed, encrypted changes to a space's log and hands them out from a cursor. It checks signatures and membership. It folds nothing and cannot read content, because it holds no keys. It does see metadata: which key is in which space, when and how much each key writes, the key generations and when they change, the rhythm of work, and who reads which space from where. It can also withhold changes or show members different subsets of a log. RFC 0002 lists what it learns from the log in [What the relay can see, and what it cannot](0002-change-and-log-format.md#what-the-relay-can-see-and-what-it-cannot), and RFC 0007 adds what it learns from its protocol, and what the server it runs on sees, in [What the relay sees](0007-relay.md#what-the-relay-sees). *(Amendment 2.)* Because the relay applies the membership rules itself, it also reads the control changes that carry them: each member's role, which control changes each one builds on, how many of a removed member's changes the remover had received, and the account behind each device and agent, across every space on that relay ([RFC 0008, "How the relay applies the rules"](0008-membership.md#how-the-relay-applies-the-rules)). *(Amendment 1.)* We design the relay to run on SQLite, Postgres, or DynamoDB with S3.
- **Convergence.** The same set of changes produces the same state on every device, in any order of arrival, computed fields included. Each field merges by the rules of its field type.
- **Island mode.** With no relay at all, you keep working. When you reconnect, every copy converges.
- **One schema, two SDKs.** You describe the model once in TypeScript. `ncdb model build` turns it into a versioned `model.lock.json`, which the Rust core and both SDKs (Rust and TypeScript) read. Schema changes are additive, and old clients keep data they do not understand yet.
- **Reads cost only their result.** Queries run only through declared indexes. You declare counts, blockers and rankings as computed fields and views, and the database updates them as it folds new changes. A list of 100 entries is one range lookup, not a hundred fetches.
- **Ordered pages.** Every index and every view keeps its order, so results arrive sorted, and you load the next page from a cursor only when you need it. "What should I work on next?" returns the first 50 ready tasks in rank order. The next 50 come when you ask for them.
- **Field types.** Every field has a type, and the type declares how concurrent changes to the field merge. NoCentralDB ships a set of field types (see [Field types we ship](#field-types-we-ship)). You can add your own as a plugin, with its own merge rules, and use it in the schema next to the built-in ones. Each member's core runs the same field types, so every copy merges the same way.
- **Work orders.** One member performs a job such as compaction or an erasure, and another member checks it. For a reversible job, a second device cross-checks it. For a job that cannot be undone, a second account signs it off. The relay sees only an opaque envelope and content-free effects.
- **Shred and audit.** The schema marks personal fields, and the SDK encrypts them with one key per person per space ([RFC 0003, "The person key"](0003-keys-and-cryptography.md#the-person-key)). Erasing a person destroys every one of those keys, once every admin device of the space has seen the erasure. *(Amendment 1.)* The person's fields become unreadable in archives, in old snapshots and on every device that has deleted the key, but not where someone kept the key: on a device that never comes back online, in a modified client that ignores the shred, or for a member of the space who obtains a relay backup taken before the shred ([RFC 0003, "Shredding a person's key"](0003-keys-and-cryptography.md#shredding-a-persons-key); [the team lead](#the-team-lead-who-has-to-pass-the-data-protection-review) below lists every limit). Compaction does not throw history away. It moves the history, encrypted and signed, into an archive, so who changed what and when stays provable. An audit is a work order that loads the archive and verifies it. We design shredding to leave the signatures intact while the person's fields become unreadable.

```ts
import { nc } from "@nocentraldb/sdk";

export const model = nc.model({
  tasks: nc.collection({
    title: nc.string(),
    status: nc.oneOf(["open", "in_progress", "done"]),
    priority: nc.int(),
    dependsOn: nc.links("tasks"),
  }, {
    indexes: { byStatusPriority: nc.index(["status", "priority"], { equal: 1 }) },
  }),
});

const query = model.tasks.indexes.byStatusPriority.equal("open").take(50);
const page = await db.find(query);                       // the first 50 open tasks, by priority
const more = await db.find(query.after(page.cursor));    // the next 50, when needed
```

Computed fields and views, such as the ready tasks that nothing blocks, in rank order, keep their reserved names `nc.reach` and `nc.view` until a later RFC designs them; they are then queried the same way, with `take` and `after`. Until then, the example queries a declared index ([RFC 0009, "Queries and indexes"](0009-sdk-api.md#queries-and-indexes)). *(Amendment 6.)*

## Architecture

The first diagram shows the components. Every member, whether a browser, a phone, a laptop, an agent or a cloud function, runs the same core with a full copy of the spaces it belongs to. The relay stores and forwards encrypted changes and holds no keys. Field types are plugins of the core. Adapters connect a space's change stream to other systems, such as EventBridge on AWS.

![Components: your app or agent on top of the TypeScript or Rust SDK, the core on every member with schema, keys, encrypted log, local SQLite view, subscriptions and field types as plugins; in the cloud the relay that holds no keys, adapters such as the AWS adapter, and servers that must read joining as regular members](0001-architecture.svg)

The second diagram follows one change through two spaces. Bob writes in the space "Project A". Every member of Project A receives the change and reads it: Alice's laptop, an agent and a cloud function. Carol is only a member of the space "HR", so she receives nothing from Project A and has no key to read it.

![How one change travels: Bob signs and encrypts a change in Project A; the relay checks his signature and membership and appends the ciphertext; Alice, Agent X and a cloud function, all members of Project A, decrypt and fold it; Carol, a member of HR only, receives nothing](0001-change-flow.svg)

## Field types we ship

Each field type defines how a field stores its value and how concurrent changes merge. NoCentralDB ships the following field types. You can add your own as a plugin.

- **Basic values.** Strings, numbers, booleans and a choice from a fixed list. The last writer wins.
- **Sets and links.** A set merges additions and removals from every member. A link points to another object in the same space, and computed fields and views can follow links, for example to find the open tasks a task depends on.
- **Rich text.** A shared document, built on Yjs, with Yrs in the core. People edit it in an editor such as TipTap, and concurrent edits merge without conflicts. Agents read it as Markdown and change it with anchored edits. An anchored edit quotes the text it changes, and the engine turns it into minimal operations, so a person typing elsewhere in the document loses nothing.
- **Stream.** A field that grows chunk by chunk, for example an agent's answer that appears word by word in the user interface. Each chunk travels over the same encrypted change stream as every other change. When the writer closes the stream, the field holds the complete value.
- **Personal fields.** A basic value can be marked as personal when the field is added, and tied to a person ([RFC 0005, "What the build checks"](0005-schema-and-model-lock.md#what-the-build-checks)). *(Amendment 4.)* The SDK then encrypts it with that person's own key, one key per person per space ([RFC 0003, "The person key"](0003-keys-and-cryptography.md#the-person-key)), so that destroying those keys makes it unreadable wherever the key is gone (see [Shred and audit](#the-design-in-one-page) for where it is not).

[Eight use cases](../use-cases.md) show what this design means in practice. In five of them, a central database is the weaker choice by design. The other three show that collaborative editing, subscriptions and streaming work without a central server. Each use case ends with a question for you. The [FAQ](../faq.md) answers the questions readers ask most often.

## Three people we are designing for

### The developer whose agents work on shared data

**Before.** Her agent picks up the next task by calling the API for open tasks, then for each task's dependencies, then for the people involved. Each step waits for the network. A planning run takes minutes and trips the rate limit.

**After.** The agent is a member of the project's space, with its own key and its own role. It asks a local view for the ready tasks and gets them in one lookup. It signs its writes with its own key, so everyone can see what the agent changed, and anyone with the right can remove the agent from the space at any time.

**What she would tell a colleague:** "The agent reads from its own copy. No round-trips, and it only ever sees the spaces we gave it."

### The team lead who has to pass the data-protection review

**Before.** For every erasure request, someone searches the production database, the backups, the logs, the search index and whatever the agents copied, and writes down where they looked.

**After.** The schema marks personal fields and ties each one to a person. An erasure is a work order: one member performs it, a second account that holds an admin member in the space signs it off, and the person's key is destroyed once every admin device has seen the erasure. *(Amendment 1.)* The person's fields in archives and old snapshots become unreadable because the key is gone. Nobody has to find every copy. The audit trail still shows who changed what, when, and who signed off the erasure.

**What he would tell his boss:** "Erasure destroys a key instead of hunting through backups, the audit trail survives it, and the design says plainly what it does not reach."

The erasure does not reach these cases:

- Devices that never come back online keep their keys and data.
- Copies made outside the database, such as exports and screenshots, stay readable.
- The database does not find personal data in unmarked free text. Keeping names out of free text is the app's job: it stores a reference to the person instead of the name and resolves the reference when it displays the text, so that after an erasure the text shows a placeholder; a name typed or pasted as plain text stays ([RFC 0005, "Personal data in free text"](0005-schema-and-model-lock.md#personal-data-in-free-text)). *(Amendment 4.)*
- Removing a member is not erasure: it blocks the member's access from then on, while the data the member wrote and the personal fields about them stay ([RFC 0005, "About whom, not by whom"](0005-schema-and-model-lock.md#about-whom-not-by-whom)). *(Amendment 4.)*
- An agent that runs as a user on that user's device can read what the user can read.
- A modified client can ignore the erasure and keep the key ([RFC 0003, "Shredding a person's key"](0003-keys-and-cryptography.md#shredding-a-persons-key)).
- Backups of the relay keep the wrapped key until they expire. The relay cannot open it, but a member of the space who obtains a backup taken before the erasure can ([RFC 0003, "Shredding a person's key"](0003-keys-and-cryptography.md#shredding-a-persons-key)).
- The key is destroyed only once the erasure is stable: every admin device of the space has written a control change after seeing it. Every view shows the person's fields as erased at once, but until then the key still exists on the members. An admin device that stays offline delays it for as long as it stays away. After 7 days the SDK of every admin raises an alarm that names the blocking devices, and an admin demotes or removes them ([RFC 0008, "Effects that wait until a change is stable"](0008-membership.md#effects-that-wait-until-a-change-is-stable)). *(Amendment 1.)*

Your data protection officer decides whether destroying a key counts as erasure in your case. This is a design, not legal advice.

### The writer who shares a document with an agent

**Before.** She is editing a proposal while an agent updates the budget section. The agent rewrites the whole document, and her last paragraph disappears into a deleted region.

**After.** The agent reads the outline, then the one section it needs, and replaces "due 12.11." with "due 19.11." as an anchored edit. The change is a few dozen bytes. Her paragraph, written at the same moment, is still there.

**What she would tell a colleague:** "The agent edits like a careful co-author. It quotes what it changes and leaves the rest alone."

## Example applications

We plan to ship example applications, so that you can try the design in a working app instead of reading about it. Each one follows one of the three people above.

1. **Agent task board.** A team's tasks with dependencies, and an agent that is a member of the project's space. The agent picks the next ready task from a local view and writes its changes under its own key. It shows spaces, membership, views, computed fields and ordered pages.
2. **Customer records with erasure.** A small customer database with personal fields. An erasure is a work order that one member performs and a second account signs off, and the audit trail survives it. It shows personal fields, work orders and the audit.
3. **Proposal editor with an agent co-author.** A shared document edited in TipTap by two people and an agent at the same time, including a stretch without network. It shows the rich-text field type, anchored edits and island mode.
4. **Agent chat.** A chat in which an agent's answer streams into the interface and a new message triggers a notification. It shows the stream field type and subscriptions.

Which example would show you whether NoCentralDB fits your app? Tell us, or propose your own (see [Where we want your advice](#where-we-want-your-advice), point 7).

## What it is not

- **Not for every app.** Apps that need queries on the server, analytics across all users' data, or one global truth enforced at write time (unique names, transactions across everyone) belong on a central database.
- **Not in v1.** Server-side queries, full-text search in the core, transactions across spaces and row-level permissions. Rights apply per space, and every member of a space holds a full copy of it.
- **A different choice, on purpose.** Some sync engines keep a trusted server that enforces fine-grained rights and can read the data. We chose a server that cannot read it. The cost is coarser rights. We offset it with many small spaces, keys that the SDK manages out of sight, and compaction instead of replaying long histories. *(Amendment 5.)*
- **Not a security boundary on your own device.** Rights bind third parties. A process running as you can read what you can read.
- **Not post-quantum in v1.** v1 wraps keys with X25519, which a future quantum computer could break. Someone who records ciphertext today, the relay's operators included, could read it once such a computer exists. A hybrid suite is planned for later ([RFC 0003, decision 9](0003-keys-and-cryptography.md#decisions-for-the-owner)).
- **Not a durable store in every browser.** A browser can evict a site's storage: under storage pressure unless the site has persistent storage, and Safari after seven days without user interaction for script-writable storage, unless the web app was added to the home screen. The browser deletes the member key with the copy, so the person sets the browser up again as a new device, which fetches its spaces again from the relay; changes the old member wrote that the relay had not yet received are lost. *(Amendment 3.)* [Browser apps and durability](../browser-durability.md) says what each browser does, what is lost when, and what to tell your users ([RFC 0006, "The browser"](0006-local-view.md#the-browser), "Limits of storage in the browser").

## Who is behind it

nxsflow started NoCentralDB. Our own products (nexus-flow, nexflow.it and manufakt.io) are meant to be its first users. In them we hit the limits of folding everyone's data on a server. We would rather design the database in the open than build it in private and ask for trust afterwards. The roadmap shows what is designed, what is being built and what is still open.

## Where we want your advice

We are deciding the points below. Each answer from your own experience helps us decide.

1. **Rights per space.** Are per-space rights, with many small spaces, enough for v1, or are field-level write scopes needed from day one? In your app, would one space per project or per shared document work, and where would it break? *(Amendment 5.)*
2. **Server-readable data.** Some features need a server that can read, for example rendering a page on the server or sending an email digest. We plan to make that server a regular member of the spaces it needs, visible in the member list, instead of offering a "server-readable" mode. Would that pass your review, or do you need something else?
3. **Erasure evidence.** For a data-protection review, would a signed-off work order plus a destroyed key be enough evidence of erasure? What else would your data protection officer ask for?
4. **Computed fields.** We plan a fixed set of declarative functions (follow, count, any, min, max, reach, cycle), and custom reducers in Rust later. Which derived value in your app would not fit that set?
5. **Nobody online.** A work order needs one member that can perform it and one that can check it. If no eligible device comes online before a deadline, such as the one-month deadline for a data-subject request, should the relay escalate to the admins, or should an operator be able to run a dedicated helper member?
6. **Hosting the relay.** Would you rather run the relay yourself on Postgres, run it serverless in your own cloud account, or use a hosted relay that can only ever see ciphertext?
7. **Example applications.** Which of the planned example applications would help you most, which is missing, and would you contribute one from your own domain?

With your answers we will decide the v1 scope of rights, the server-member model, the erasure evidence, the computed-field set, the escalation path for work orders, which relay backend comes first, and which example applications we build first.

## How to give advice

Reply in [this RFC's discussion](https://github.com/nocentraldb/nocentraldb/discussions/21) on GitHub, one topic per reply, or start a new discussion in the category [RFC feedback](https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback) when your point needs room of its own. Tell us about your app, your situation and where the design would break for you. Reports of problems help us more than feature requests. Check the [FAQ](../faq.md) first; your question may already have an answer. To propose a change to the text itself, open a pull request against this file.

We read every reply, answer within a week, and publish a summary of the changes in the next draft. [What happens to your feedback](README.md#what-happens-to-your-feedback) says how a point becomes an amendment, a ticket or an answer. Watch the repository's releases to hear when that draft is out.

## Amendment 1: following RFC 0008

Added on 2026-11-08 for NOC-44. [RFC 0008](0008-membership.md) defines membership, roles and the first change of a space, and the owner accepted every one of its decisions, M1 to M26 (Carsten, 2026-11-08). Its section [Conflicts with GENESIS.md and the other RFCs](0008-membership.md#conflicts-with-genesismd-and-the-other-rfcs) names three points where this RFC said something else. This amendment changes those sentences, marked *(Amendment 1)* above, and decides nothing new:

- **Groups** (conflict 1, decision M10). This RFC said groups can be members of many spaces. RFC 0008 leaves a group as a member of its own to a later RFC: a group's membership would live outside the space, and one space's fold would then depend on another space's log. Until then, a group is a list of members that the SDK expands into single members, and removing a member from the list removes it from every space the SDK added it to through the list. Rights still change without moving data. What v1 loses is one entry that changes many spaces at once ([RFC 0008, "Groups"](0008-membership.md#groups)). *Withdrawn by the owner (Carsten, 2026-11-09) in [Amendment 5](#amendment-5-groups-leave-the-design).*
- **What the relay sees** (conflict 2, decision M11). The relay applies the membership rules, so it reads the bodies of control changes and which control changes each one builds on. It learns each member's role and when it changes, the cutoffs of revocations, which control changes each admin had seen, the account key behind each member across spaces, the sub-agent tree, which accounts sign off erasures, and when concurrent rotations happened. It still learns no content, no space name and no model id ([RFC 0008, "How the relay applies the rules"](0008-membership.md#how-the-relay-applies-the-rules)).
- **When an erasure destroys the key** (conflict 15, decisions M20 and M21). Destroying a person key, the "deleted" acknowledgement and the relay's deletion of its record wait until the shred is stable, because a concurrent revocation could still reject it and a destroyed key cannot come back. Every view shows the person's fields as erased at once. An admin device that does not come online blocks the destruction; after 7 days the SDK of every admin raises an alarm naming it, and an admin demotes or removes it. The list of what the erasure does not reach names this wait ([RFC 0008, "Effects that wait until a change is stable"](0008-membership.md#effects-that-wait-until-a-change-is-stable), and [decisions M20 and M21](0008-membership.md#decisions-for-the-owner)).

## Amendment 2: following RFC 0007

Added on 2026-11-09 for NOC-47. [RFC 0007](0007-relay.md) defines the relay's protocol and its AWS backend, and the owner ruled on every point of its section [Conflicts with GENESIS.md and RFC 0001](0007-relay.md#conflicts-with-genesismd-and-rfc-0001) (Carsten, 2026-11-08). This amendment changes the one sentence that needs, marked *(Amendment 2)* above. The owner has decided it, so it adds no decision.

- **What the relay sees** (RFC 0007's conflict 1). This RFC said RFC 0002 lists all of what the relay sees. RFC 0002's list names what the relay learns from the log. [RFC 0007, "What the relay sees"](0007-relay.md#what-the-relay-sees) adds what it learns from its protocol, such as when each member is online, which client it runs and which spaces it follows from which cursor, and separates all of it from what the server the relay runs on sees. The relay bullet now links both lists, so that neither reads as complete on its own.

## Amendment 3: a browser copy takes its member key with it

Added on 2026-11-09 for NOC-34. NOC-33 measured and collected what browsers do with a web app's copy ([How long a browser keeps a web app's copy](../browser-storage.md)). A browser evicts all of a site's data at once, and Safari's seven-day deletion names IndexedDB, where the member key lives in a browser ([RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys)). This amendment changes the one sentence that claimed the browser stays the same member, marked *(Amendment 3)* above. It follows [RFC 0006, "Durability of a browser copy"](0006-local-view.md#durability-of-a-browser-copy) and decides nothing new.

- **Not a durable store in every browser.** This RFC said that after an eviction the member fetches the log of its spaces again. Without its member key, the browser is no longer that member: the person pairs it again as a new device ([RFC 0003, "The account key"](0003-keys-and-cryptography.md#the-account-key)), which then fetches its spaces. The bullet now says so and links the developer page [Browser apps and durability](../browser-durability.md).

## Amendment 4: following RFC 0005 on personal fields

Added on 2026-11-09 for NOC-35. [RFC 0005](0005-schema-and-model-lock.md) defines how the schema marks personal fields, and the owner accepted every one of its decisions (Carsten, 2026-11-08). This amendment changes the sentences that said more or less than RFC 0005 decided, marked *(Amendment 4)* above. It follows RFC 0005 and decides nothing new.

- **Which fields can be personal** (decision D9). This RFC said any field can be marked as personal. In v1, only basic values can: a set compares its elements by their bytes, and a sealed value has different bytes each time it is written; a link must stay readable to resolve its target. A field is personal from the version that adds it, or never ([RFC 0005, "Personal fields"](0005-schema-and-model-lock.md#personal-fields)).
- **Personal data in free text** (decision D15). This RFC named unmarked free text as a limit of erasure and stopped there. NoCentralDB does not and cannot find a name inside free text, so keeping names out of it is the app's job, and RFC 0005 gives the pattern: store a reference to the person and resolve it at display time ([RFC 0005, "Personal data in free text"](0005-schema-and-model-lock.md#personal-data-in-free-text)).
- **Removing a member is not erasure.** The list of what erasure does not reach now says so: removing a member blocks its access from then on, and the data it wrote and the personal fields about it stay ([RFC 0005, "About whom, not by whom"](0005-schema-and-model-lock.md#about-whom-not-by-whom)).

## Amendment 5: groups leave the design

Added on 2026-11-09 for NOC-76. The owner decided to keep groups out of every definition for now (Carsten, 2026-11-09): they caused more problems than they solved. If a later need shows they are useful, they come back through a later RFC. The owner removed the sentence on groups from GENESIS.md too. This amendment changes the sentences that named groups, marked *(Amendment 5)* above, and the point on groups in [Amendment 1](#amendment-1-following-rfc-0008) is withdrawn. It records the owner's decision and decides nothing else.

- **Spaces.** A member is a person's device, an agent or a sub-agent. There is no group, neither a group that is a member of spaces nor a list of members that the SDK expands. An app that gives several members the same spaces adds each of them to each space. [RFC 0008, Amendment 1](0008-membership.md#amendment-1-groups-leave-the-design) withdraws its decision M10, and [RFC 0009, Amendment 1](0009-sdk-api.md#amendment-1-groups-leave-the-design) its decision S14.
- **What softens coarse rights.** "A different choice, on purpose" and question 1, "Rights per space", no longer offer groups the SDK expands. Many small spaces remain.

## Amendment 6: following RFC 0009

Added on 2026-11-09 for NOC-75. [RFC 0009](0009-sdk-api.md) defines the SDK's API, and the owner accepted every one of its decisions, S1 to S18 (Carsten, 2026-11-09). Its section [Conflicts with GENESIS.md and the other RFCs](0009-sdk-api.md#conflicts-with-genesismd-and-the-other-rfcs) names one point where this RFC's example cannot run. This amendment changes the example and adds the sentence after it, marked *(Amendment 6)* above, and decides nothing.

- **`db.find` on a view** (RFC 0009's [point 6](0009-sdk-api.md#conflicts-with-genesismd-and-the-other-rfcs), decision S7). The example declared a computed field with `nc.reach` and a view with `nc.view` and queried the view. RFC 0005 reserves both names but does not design them, so no query can run on a view yet. The example now declares an index and queries it, as RFC 0009 defines it; the name `db.find` and the calls `take` and `after` stay, and a view is queried the same way once it is designed.
