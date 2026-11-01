# NoCentralDB use cases

Eight situations that show what NoCentralDB is for. The first five are cases where a central database is the weaker choice by design, not only by effort. The last three answer the obvious objection: "local-first is nice, but real-time collaboration, notifications and streaming need a server." They don't.

**Status:** NoCentralDB is not released yet. The design is in [RFC 0001](rfc/0001-nocentraldb.md), and this page describes how that design would play out. The code below is the API **we propose**, not a finished one. Every use case ends with a question, and your answer is the most useful thing you can send us. See [How to tell us](#how-to-tell-us).

Each use case follows the same shape: the situation, what goes wrong with a central database (or, in part 2, why NoCentralDB handles it just as well), what it looks like in your app, what it does not cover, and our question to you.

**Part 1: Why NoCentralDB instead of a central database**

1. [Erasure that actually erases](#1-erasure-that-actually-erases)
2. [The breach that leaks nothing readable](#2-the-breach-that-leaks-nothing-readable)
3. [The outside agent that sees one room](#3-the-outside-agent-that-sees-one-room)
4. [A thousand agent queries a minute, at no cost](#4-a-thousand-agent-queries-a-minute-at-no-cost)
5. [The relay goes down and nobody notices](#5-the-relay-goes-down-and-nobody-notices)

**Part 2: And you give up nothing**

6. [Writing together through a tunnel](#6-writing-together-through-a-tunnel)
7. [Subscribe instead of poll](#7-subscribe-instead-of-poll)
8. [Streaming as a data type](#8-streaming-as-a-data-type)

---

## Part 1: Why NoCentralDB instead of a central database

### 1. Erasure that actually erases

**The situation.** A former customer asks you to delete their personal data. Your auditor also wants proof of who changed what over the last two years.

**With a central database** you go searching: the production database, every backup, the logs, the search index, the copies your agents made. Then you write down where you looked. Deleting history and keeping an audit trail pull in opposite directions. Backups make "deleted" something you can only claim, not prove.

**With NoCentralDB** the schema marks personal fields and ties them to a person, and each person's fields are encrypted with that person's own key. Erasure is a work order: one member performs it, a second account signs it off, and the key is destroyed. From then on, every copy of those fields is unreadable, on every device and in every archive and old snapshot, because the key is gone, not because someone found every copy. The signatures stay intact, so the audit trail still shows who changed what and when, and who signed off the erasure.

**In your app:**

```ts
export const model = nc.model({
  customers: nc.collection({
    name: nc.string({ personal: true }),      // encrypted under the person's key
    email: nc.string({ personal: true }),
    plan: nc.oneOf(["free", "team"]),          // not personal, stays readable
  }, { person: "self" }),
});

await db.workOrders.erase({ person: customerId });  // needs a second account's sign-off
```

**What it does not cover.**
- Devices that never come back online keep their keys and their data.
- Copies made outside the database (exports, screenshots) are not covered.
- Personal data in unmarked free text is not found.

Whether destroying a key counts as erasure in your case is for your data protection officer to decide. This is a design, not legal advice.

**Tell us:** Would a signed-off work order plus a destroyed key be enough evidence for your data protection officer, and what would they ask for in addition?

### 2. The breach that leaks nothing readable

**The situation.** Someone copies the relay's entire database: a compromised server, a stolen backup, a curious operator. In the video, you see that dump, and it is ciphertext from start to finish.

**With a central database** the server has to read the data to filter it, index it and enforce rights on it. Row-level rights protect users from each other, not from the server, its operators, its backups or the next breach. Each breach of the server is a breach of everything it stores.

**With NoCentralDB** the relay only appends signed, encrypted changes to each space's log and hands them out. It checks signatures and membership, but it holds no keys and folds nothing. Every member decrypts and folds its own copy. Nothing on the relay can be read, so a leak of the relay leaks nothing readable.

**In your app:** nothing to write. Encryption is not a feature you switch on; the SDK never hands your data to the relay unencrypted, and keys never leave the members' devices.

**What it does not cover.** The relay sees metadata: which key is in which space, and when and how much each one writes. A member's own device is not protected against that member: a process running as you can read what you can read.

**Tell us:** Would "the relay can only ever see ciphertext and this metadata" pass your security review, and which piece of metadata would worry you?

### 3. The outside agent that sees one room

**The situation.** A contractor, or a third party's AI agent, should work on one project and nothing else. When the engagement ends, it should be out.

**With a central database** the agent's access depends on row-level security and on trusting whoever runs the server. One wrong policy and it sees every customer. Taking access away means getting every policy right again.

**With NoCentralDB** the agent becomes a member of exactly one space, with its own key and role. It cannot decrypt any other space, because it never received those keys. Every write it makes carries its own signature, so everyone sees what the agent changed. When the engagement ends, you remove it from the space and the space's key rotates. From then on it receives nothing new.

**In your app:**

```ts
const project = await db.spaces.get(projectId);
await project.members.add(agentPublicKey, { role: "editor" });

// later
await project.members.remove(agentPublicKey);  // rotates the space key
```

**What it does not cover.** Removal stops the future, not the past: what the agent already received stays in its copy. Rights are per space. A part of a project the agent must not see belongs in a space of its own.

**Tell us:** In your app, would one space per project or per shared document be workable, and where would it break?

### 4. A thousand agent queries a minute, at no cost

**The situation.** An agent plans its work in a loop. "What should I work on next?" It reads the ready tasks with their blockers, re-ranks them, takes the next page, and asks again.

**With a central database** every one of those questions is a round-trip: latency, a rate limit, load on a server you share with your users, and a bill. The work itself is fast; the waiting is not. Agents ask in volumes central databases were never sized for.

**With NoCentralDB** the agent asks its own local copy. Counts, blockers and rankings are computed fields and views that are kept up to date as changes arrive. A page of 50 ready tasks is one range lookup, already in rank order. Nothing goes over the network, so there is nothing to rate-limit and nothing to bill.

**In your app:**

```ts
const page = await db.find(model.tasks.views.ready.take(50));                     // first 50, ranked
const more = await db.find(model.tasks.views.ready.take(50).after(page.cursor));  // next 50
```

**What it does not cover.** Queries run only through declared indexes and views; there are no ad-hoc queries on the server and no analytics across all users' data. For those, a central database is still the right place.

**Tell us:** Which derived value in your app would not fit our planned set of computed-field functions (follow, count, any, min, max, reach, cycle)?

### 5. The relay goes down and nobody notices

**The situation.** The relay is unreachable for an hour. Or you move it from SQLite on a small server to DynamoDB in your own cloud account.

**With a central database** an outage stops the app: no reads, no writes. A move means a migration and a maintenance window.

**With NoCentralDB** every member keeps working on its full local copy during the outage, reading and writing. When the relay is back, every copy converges to the same state, whatever order the changes arrive in. Members on the same network can keep exchanging changes in the meantime, which we call island mode. The relay stores only an append-only log of encrypted changes, so it is the simplest part of the system to replace.

**In your app:** nothing. Reads never leave the device, and writes queue until the relay answers.

**What it does not cover.** Changes made during the outage reach the other side only when a path exists again. Anything that needs one global truth at write time, such as unique names or a transaction across everyone, does not fit this model. We also have not designed yet how a move between relay backends is carried out. That depends on your answer below.

**Tell us:** Would you rather run the relay yourself (Postgres), serverless in your own cloud account, or use a hosted relay that, by design, can only ever see ciphertext?

---

## Part 2: And you give up nothing

### 6. Writing together through a tunnel

**The situation.** Two people in two cities edit the same document. Each sees the other's typing with a short delay. Then one of them is on a train, and the train goes into a tunnel. Both keep typing. Their changes no longer reach each other. When the train comes out, the other person's paragraphs appear on each screen, and nobody's text is lost.

**Why NoCentralDB handles it.** Long text is a shared document (Yjs, with Yrs in the Rust core). Each person edits their own local copy, which is why typing never waits for the network. When they reconnect, both copies converge without conflicts. Agents can take part too: they read the document as Markdown and change it with anchored edits that quote what they change. A person typing elsewhere in the document loses nothing.

**In your app:** the rich-text field type ships with NoCentralDB, ready for TipTap:

```ts
export const model = nc.model({
  proposals: nc.collection({
    title: nc.string(),
    body: nc.richText(),   // a shared document, ready for TipTap
  }),
});

const doc = await db.get(model.proposals, proposalId);
const editor = new Editor({ extensions: [StarterKit, nc.tiptap(doc.body)] });
```

Rich text is one field type among others. You can define your own field types, with their own merge semantics, and use them in your schema next to the built-in ones.

**Tell us:** Which editor or collaborative data type would you want to plug in, and what would it need from us?

### 7. Subscribe instead of poll

**The situation.** A user gets a new chat message, and the app shows a notification at once. Nobody polls.

**Why NoCentralDB handles it.** Every member already receives the changes of the spaces it belongs to. A subscription is a filter over that stream: the app says which changes it cares about and is called when one arrives.

**In your app:**

```ts
db.subscribe(model.messages, { where: { to: me, read: false } }, (message) => {
  notify(`New message from ${message.from.name}`);
});
```

**In the cloud**, the same works for agents that should only run when something happens. An AWS plugin feeds a space's stream into EventBridge. There, a small function decrypts each change and matches it against the declared subscriptions, and the agent is invoked only on a match. All of it is serverless: no polling, and no cost while nothing happens. That function is a regular member of the space, with its own key, and is visible in the member list like everyone else. There is no hidden "server-readable" mode.

**Tell us:** Would a cloud function that is a visible member of the space, with its own key, pass your review, or do you need something else?

### 8. Streaming as a data type

**The situation.** A user chats with an agent. The answer appears word by word as the agent writes it, the way people are used to.

**Why NoCentralDB handles it.** The answer is a field of a streaming type. The agent appends chunks to it, and every chunk travels over the same encrypted change stream as everything else. There is no separate socket, no second channel to secure and no second place where the conversation lives. When the stream ends, the field holds the complete answer like any other value.

**In your app** with TanStack AI:

```ts
export const model = nc.model({
  messages: nc.collection({
    role: nc.oneOf(["user", "agent"]),
    content: nc.stream(),   // appended chunk by chunk, complete when closed
  }),
});

// the agent
const reply = await db.insert(model.messages, { role: "agent" });
for await (const chunk of chat({ adapter, messages })) {
  await reply.content.append(chunk);
}
await reply.content.close();

// the UI
const { content } = nc.useLive(model.messages, replyId);
```

**Tell us:** Should every chunk stay in the log forever, or should a finished stream be compacted into its final text, and what would your app need?

---

## How to tell us

Tell us in the [RFC discussions](https://github.com/nocentraldb/nocentraldb/discussions) on GitHub. Describe your app, your situation and where the design would break for you; answers about problems help us more than feature requests. We read every reply, answer within a week, and publish a summary of what changed in the next draft of the RFC.
