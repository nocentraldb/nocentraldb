# RFC 0001: NoCentralDB

REQUEST FOR COMMENTS · DRAFT 0.1 · OCTOBER 2026 · nocentraldb.io

A local-first database for people and agents.

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

- **Spaces.** A space is the unit of sharing, membership and keys. Every object belongs to exactly one space. Fine-grained rights come from many small spaces. Groups can be members of many spaces, so you can rearrange rights without moving data.
- **A full copy per member.** Each device and each agent keeps the complete, encrypted log of every space it belongs to, and folds it locally into an SQLite view. Reads never leave the device.
- **A relay that holds no keys.** The relay appends signed, encrypted changes to a space's log and hands them out from a cursor. It checks signatures and membership. It folds nothing and cannot read content. It can see metadata: which key is in which space, and when and how much each key writes. We design the relay to run on SQLite, Postgres, or DynamoDB with S3.
- **Convergence.** The same set of changes produces the same state on every device, in any order of arrival, computed fields included. Each field merges by its declared semantics: last writer wins, sets, links or shared text.
- **Island mode.** With no relay at all, you keep working. When you reconnect, every copy converges.
- **One schema, two SDKs.** You describe the model once in TypeScript. `ncdb model build` turns it into a versioned `model.lock.json`, which the Rust core and both SDKs (Rust and TypeScript) read. Schema changes are additive, and old clients keep data they do not understand yet.
- **Reads cost only their result.** Queries run only through declared indexes. You declare counts, blockers and rankings as computed fields and views, and the database updates them as it folds new changes. A list of 100 entries is one range lookup, not a hundred fetches.
- **Ordered pages.** Every index and every view keeps its order, so results arrive sorted, and you load the next page from a cursor only when you need it. "What should I work on next?" returns the first 50 ready tasks in rank order. The next 50 come when you ask for them.
- **Collision-free edits.** Long text is a shared document, built on Yjs, with Yrs in the core. Agents read it as Markdown and change it with anchored edits. An anchored edit quotes the text it changes, and the engine turns it into minimal operations, so a person typing elsewhere in the document loses nothing.
- **Work orders.** One member performs a job such as compaction or an erasure, and another member checks it. For a reversible job, a second device cross-checks it. For a job that cannot be undone, a second account signs it off. The relay sees only an opaque envelope and content-free effects.
- **Shred and audit.** The schema marks personal fields, and the SDK encrypts them with one key per person. Erasing a person destroys that key, and the erasure reaches archives and old snapshots too. Compaction does not throw history away. It moves the history, encrypted and signed, into an archive, so who changed what and when stays provable. An audit is a work order that loads the archive and verifies it. We design shredding to leave the signatures intact while the person's fields become unreadable.

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

[Eight use cases](../use-cases.md) show what this design means in practice. In five of them, a central database is the weaker choice by design. The other three show that collaborative editing, subscriptions and streaming work without a central server. Each use case ends with a question for you. The [FAQ](../faq.md) answers the questions readers ask most often.

## Three people we are designing for

### The developer whose agents work on shared data

**Before.** Her agent picks up the next task by calling the API for open tasks, then for each task's dependencies, then for the people involved. Each step waits for the network. A planning run takes minutes and trips the rate limit.

**After.** The agent is a member of the project's space, with its own key and its own role. It asks a local view for the ready tasks and gets them in one lookup. It signs its writes with its own key, so everyone can see what the agent changed, and anyone with the right can remove the agent from the space at any time.

**What she would tell a colleague:** "The agent reads from its own copy. No round-trips, and it only ever sees the spaces we gave it."

### The team lead who has to pass the data-protection review

**Before.** For every erasure request, someone searches the production database, the backups, the logs, the search index and whatever the agents copied, and writes down where they looked.

**After.** The schema marks personal fields and ties each one to a person. An erasure is a work order: one member performs it, a second account signs it off, and the person's key is destroyed. The person's fields in archives and old snapshots become unreadable because the key is gone. Nobody has to find every copy. The audit trail still shows who changed what, when, and who signed off the erasure.

**What he would tell his boss:** "Erasure destroys a key instead of hunting through backups, the audit trail survives it, and the design says plainly what it does not reach."

The erasure does not reach these cases:

- Devices that never come back online keep their keys and data.
- Copies made outside the database, such as exports and screenshots, stay readable.
- The database does not find personal data in unmarked free text.
- An agent that runs as a user on that user's device can read what the user can read.

Your data protection officer decides whether destroying a key counts as erasure in your case. This is a design, not legal advice.

### The writer who shares a document with an agent

**Before.** She is editing a proposal while an agent updates the budget section. The agent rewrites the whole document, and her last paragraph disappears into a deleted region.

**After.** The agent reads the outline, then the one section it needs, and replaces "due 12.11." with "due 19.11." as an anchored edit. The change is a few dozen bytes. Her paragraph, written at the same moment, is still there.

**What she would tell a colleague:** "The agent edits like a careful co-author. It quotes what it changes and leaves the rest alone."

## What it is not

- **Not for every app.** Apps that need queries on the server, analytics across all users' data, or one global truth enforced at write time (unique names, transactions across everyone) belong on a central database.
- **Not in v1.** Server-side queries, full-text search in the core, transactions across spaces and row-level permissions. Rights apply per space, and every member of a space holds a full copy of it.
- **A different choice, on purpose.** Some sync engines keep a trusted server that enforces fine-grained rights and can read the data. We chose a server that cannot read it. The cost is coarser rights. We offset it with many small spaces and groups, keys that the SDK manages out of sight, and compaction instead of replaying long histories.
- **Not a security boundary on your own device.** Rights bind third parties. A process running as you can read what you can read.

## Who is behind it

nxsflow started NoCentralDB. Our own products (nexus-flow, nexflow.it and manufakt.io) are meant to be its first users. In them we hit the limits of folding everyone's data on a server. We would rather design the database in the open than build it in private and ask for trust afterwards. The roadmap shows what is designed, what is being built and what is still open.

## Where we want your advice

We are deciding the points below. Each answer from your own experience helps us decide.

1. **Rights per space.** Are per-space rights, with many small spaces and groups, enough for v1, or are field-level write scopes needed from day one? In your app, would one space per project or per shared document work, and where would it break?
2. **Server-readable data.** Some features need a server that can read, for example rendering a page on the server or sending an email digest. We plan to make that server a regular member of the spaces it needs, visible in the member list, instead of offering a "server-readable" mode. Would that pass your review, or do you need something else?
3. **Erasure evidence.** For a data-protection review, would a signed-off work order plus a destroyed key be enough evidence of erasure? What else would your data protection officer ask for?
4. **Computed fields.** We plan a fixed set of declarative functions (follow, count, any, min, max, reach, cycle), and custom reducers in Rust later. Which derived value in your app would not fit that set?
5. **Nobody online.** A work order needs one member that can perform it and one that can check it. If no eligible device comes online before a deadline, such as the one-month deadline for a data-subject request, should the relay escalate to the admins, or should an operator be able to run a dedicated helper member?
6. **Hosting the relay.** Would you rather run the relay yourself on Postgres, run it serverless in your own cloud account, or use a hosted relay that can only ever see ciphertext?

With your answers we will decide the v1 scope of rights, the server-member model, the erasure evidence, the computed-field set, the escalation path for work orders and which relay backend comes first.

## How to give advice

Join the discussion of this RFC in the repository's RFC discussions on GitHub. Tell us about your app, your situation and where the design would break for you. Reports of problems help us more than feature requests. Check the [FAQ](../faq.md) first; your question may already have an answer. To propose a change to the text itself, open a pull request against this file.

We read every reply, answer within a week, and publish a summary of the changes in the next draft. Watch the repository's releases to hear when that draft is out.
