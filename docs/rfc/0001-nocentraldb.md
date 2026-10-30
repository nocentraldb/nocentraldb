# RFC 0001: NoCentralDB

REQUEST FOR COMMENTS · DRAFT 0.1 · OCTOBER 2026 · nocentraldb.io

A local-first database for people and agents.

## For when a central database is the wrong place.

Everyone works on their own copy, under their own keys. The relay in the middle can't read a thing.

**Status:** Not released yet. We have started building the core, and this is the design we are building toward. We want it challenged while it is still cheap to change; the roadmap shows where we are.

Agents now read and write the same data as the people they work with, hundreds of records per task. Today almost every app routes each of those reads through a server, and that server can read everything it stores. NoCentralDB is a local-first database designed for teams of people and agents: every member, human or agent, works on a full local copy of the spaces they belong to, under their own keys, and a relay that holds no keys carries the encrypted changes between them.

## The problem

You are building an app where people and agents work on the same data: tickets, notes, customer records, a shared document. Three things go wrong with the usual setup.

- **Agents are slow on a network.** An agent that wants "the next 50 open tasks, with their blockers" makes dozens of calls, waits on each, and hits rate limits. The work is fast; the round-trips are not.
- **The server in the middle can read everything.** Row-level rights protect users from each other, not from the server, its operators, its backups or the next breach. You have to explain that to your security review every time.
- **Erasure is a search.** When someone asks you to delete their data, you go looking: the database, the backups, the logs, the search index, the copies your agents made.

For many apps a central database is still exactly right. If you need queries on the server, analytics across all users' data, or one global truth enforced at write time, keep it. This RFC is for the apps where it is the wrong place.

## Why now

Two things changed. Agents became members of teams: they run on laptops, in sandboxes and on servers, and they need the same data their people use, at memory speed. And devices became able to carry the weight: a browser can hold a non-exportable key unlocked by a passkey, a phone can store a team's complete working set, and one core written in Rust can run natively and as WebAssembly in the browser.

What stops being good enough is "trust the server" as the default security model. Sometimes you don't need SQL. Sometimes you don't need a central database. Your data should live where the work happens: on every device and with every agent you trust, under your own keys. The cloud carries it. It never owns it.

## The design in one page

- **Spaces.** A space is the unit of sharing, membership and keys. Every object belongs to exactly one space. Fine-grained rights come from many small spaces; groups can be members of many spaces, so rights can be rearranged without moving data.
- **A full copy per member.** Each device and each agent keeps the complete, encrypted log of every space it belongs to, and folds it locally into an SQLite view. Reads never leave the device.
- **A relay that holds no keys.** The relay appends signed, encrypted changes to a space's log and hands them out from a cursor. It checks signatures and membership; it folds nothing and cannot read content. It can see metadata: which key is in which space, when and how much is written. Backends are designed to be SQLite, Postgres, or DynamoDB with S3.
- **Convergence.** The same set of changes gives the same state on every device, in any order of arrival, computed fields included. Fields merge by declared semantics: last writer wins, sets, links, shared text.
- **Island mode.** With no relay at all, you keep working. When you rejoin, every copy converges.
- **One schema, two SDKs.** You describe the model once in TypeScript; `ncdb model build` turns it into a versioned `model.lock.json` that the Rust core and both SDKs (Rust and TypeScript) read. Schema changes are additive; old clients keep data they do not understand yet.
- **Reads cost what the result costs.** Queries only run through declared indexes. Counts, blockers and rankings are declared as computed fields and views, maintained incrementally while folding. A list of 100 entries is one range lookup, not a hundred fetches.
- **Ordered pages.** Every index and every view keeps its order, so results arrive already sorted, and you load the next page from a cursor only when you need it. "What should I work on next?" returns the first 50 ready tasks in rank order; the next 50 come when you ask for them.
- **Collision-free edits.** Long text is a shared document (Yjs, Yrs in the core). Agents read it as Markdown and change it with anchored edits: they quote what they change, and the engine turns that into minimal operations, so a person typing elsewhere in the document loses nothing.
- **Work orders.** Jobs such as compaction or an erasure are performed by one member and checked by another (a cross-check by a second device, or a sign-off by a second account for what cannot be undone). The relay only sees an opaque envelope and content-free effects.
- **Shred and audit.** Personal fields are marked in the schema and encrypted with one key per person. Erasing a person destroys that key, which also reaches archives and old snapshots. Compacted history is not thrown away: it moves, encrypted and signed, into an archive, so who changed what and when stays provable. An audit is a work order that loads the archive and verifies it, and shredding is designed to leave the signatures intact while the person's fields become unreadable.

```ts
import { nc } from "@nocentraldb/sdk";

export const model = nc.model({
  tasks: nc.collection({
    title: nc.string(),
    status: nc.oneOf(["open", "in_progress", "done"]),
    priority: nc.int(),
    dependsOn: nc.links("tasks"),
  }, {
    computed: { blocked: nc.reach("dependsOn", { status: ["open", "in_progress"] }) },
    views: { ready: nc.view("tasks", { where: { blocked: false }, sort: ["priority"] }) },
  }),
});

const page = await db.find(model.tasks.views.ready.take(50));             // first 50, in rank order
const more = await db.find(model.tasks.views.ready.take(50).after(page.cursor)); // the next 50, when needed
```

## Three people we are designing for

### The developer whose agents work on shared data

**Before.** Her agent picks up the next task by calling the API for open tasks, then for each task's dependencies, then for the people involved. Each step waits on the network; a planning run takes minutes and trips the rate limit.

**After.** The agent is a member of the project's space with its own key and its own role. It asks a local view for the ready tasks and gets them in one lookup. Its writes are signed by its own key, so everyone can see what the agent changed and the agent can be removed from the space at any time.

**What she would tell a colleague:** "The agent reads from its own copy. No round-trips, and it only ever sees the spaces we gave it."

### The team lead who has to pass the data-protection review

**Before.** For every erasure request, someone searches the production database, the backups, the logs, the search index and whatever the agents copied, and writes down where they looked.

**After.** Personal fields are marked in the schema and tied to a person. An erasure is a work order: one member performs it, a second account signs it off, and the person's key is destroyed. Archives and old snapshots become unreadable for that person, because the key is gone, not because someone found every copy. The audit trail still shows who changed what, when, and who signed the erasure off.

**What he would tell his boss:** "Erasure destroys a key instead of hunting through backups, the audit trail survives it, and the design says plainly what it does not reach."

What it does not reach: devices that never come back online keep their keys and data; copies made outside the database (exports, screenshots) are not covered; personal data in unmarked free text is not found; an agent that runs as a user on that user's device can read what the user can read. Whether destroying a key counts as erasure in your case is for your data protection officer to decide; this is a design, not legal advice.

### The writer who shares a document with an agent

**Before.** She is editing a proposal while an agent updates the budget section. The agent rewrites the whole document; her last paragraph disappears into a deleted region.

**After.** The agent reads the outline, then the one section it needs, and replaces "due 12.11." with "due 19.11." as an anchored edit. The change is a few dozen bytes. Her paragraph, written at the same moment, is still there.

**What she would tell a colleague:** "The agent edits like a careful co-author: it quotes what it changes and leaves the rest alone."

## What it is not

- **Not for** apps that need queries on the server, analytics across all users' data, or one global truth enforced at write time (unique names, transactions across everyone). For those, a central database is exactly right.
- **Not in v1:** server-side queries, full-text search in the core, transactions across spaces, row-level permissions. Rights are per space; every member of a space holds a full copy of it.
- **A different choice, on purpose.** Some sync engines keep a trusted server that enforces fine-grained rights and can read the data. We chose a server that cannot read it. The cost is coarser rights; we pay it with many small spaces and groups, keys that stay invisible in the SDK, and compaction instead of replaying long histories.
- **Not a security boundary on your own device.** Rights bind third parties. A process running as you can read what you can read.

## Who is behind it

NoCentralDB is initiated by nxsflow. Our own products (nexus-flow, nexflow.it and manufakt.io) are meant to be its first consumers; they are where we ran into the limits of folding everyone's data on a server. We would rather design the database in the open than build it behind closed doors and ask for trust afterwards. The roadmap page shows what is designed, what is being built and what is still open.

## Where we want your advice

We are deciding the points below. Each answer from your own experience helps us decide.

1. **Rights per space.** We are deciding whether per-space rights (with many small spaces and groups) are enough for v1, or whether field-level write scopes are needed from day one. In your app, would one space per project or per shared document be workable, and where would it break?
2. **Server-readable data.** Some features need a server that can read (rendering a page on the server, sending an email digest). We plan to make that server a regular member of the spaces it needs, visible in the member list, instead of offering a "server-readable" mode. Would that pass your review, or do you need something else?
3. **Erasure evidence.** For a data-protection review, would a signed-off work order plus a destroyed key be enough evidence of erasure, or what would your data protection officer ask for in addition?
4. **Computed fields.** We plan a fixed set of declarative functions (follow, count, any, min, max, reach, cycle) and custom reducers in Rust later. Which derived value in your app would not fit that set?
5. **Nobody online.** A work order needs a member that can perform it and one that can check it. If no eligible device comes online before a deadline (such as the one-month deadline for a data-subject request), should the relay escalate to the admins, or should an operator be able to run a dedicated helper member?
6. **Hosting the relay.** Would you rather run the relay yourself (Postgres), serverless in your own cloud account, or use a hosted relay that, by design, can only ever see ciphertext?

With your answers we will decide the v1 scope of rights, the server-member model, the erasure evidence, the computed-field set, the escalation path for work orders and which relay backend comes first.

## How to give advice

Join the discussion of this RFC on GitHub, in the repository's RFC discussions. Tell us about your app, your situation and where the design would break for you; answers about problems help us more than feature requests. If you want to propose a change to the text itself, open a pull request against this file.

We read every reply, answer within a week, and publish a summary of what changed in the next draft. Watch the repository's releases to hear when that draft is out.
