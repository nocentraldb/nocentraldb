# NoCentralDB use cases

These eight situations show what NoCentralDB is for. In the first five, a central database is the weaker choice by design. In the last three, people often assume that real-time collaboration, notifications and streaming need a central server. NoCentralDB handles them without one.

NoCentralDB is not released yet. [RFC 0001](rfc/0001-nocentraldb.md) describes the design, and this page shows what that design means in practice. The code on this page is an API we propose, and it may change. Every use case ends with a question for you. Your answer helps us more than anything else you could send. [How to tell us](#how-to-tell-us) explains where to send it.

Each use case has five parts:

- the situation;
- what goes wrong with a central database, or in part 2, how NoCentralDB handles the case;
- what an app writes;
- what the design does not cover;
- a question for you.

**Part 1. Why NoCentralDB instead of a central database**

1. [Erasure that reaches every copy](#1-erasure-that-reaches-every-copy)
2. [A breach of the relay leaks nothing readable](#2-a-breach-of-the-relay-leaks-nothing-readable)
3. [An outside agent limited to one space](#3-an-outside-agent-limited-to-one-space)
4. [A thousand agent queries a minute, reads at no cost](#4-a-thousand-agent-queries-a-minute-reads-at-no-cost)
5. [The relay goes down, and everyone keeps working](#5-the-relay-goes-down-and-everyone-keeps-working)

**Part 2. What you keep**

6. [Writing together through a tunnel](#6-writing-together-through-a-tunnel)
7. [Subscribe instead of poll](#7-subscribe-instead-of-poll)
8. [Streaming as a data type](#8-streaming-as-a-data-type)

---

## Part 1. Why NoCentralDB instead of a central database

### 1. Erasure that reaches every copy

**The situation.** A former customer asks you to delete their personal data. Your auditor also wants proof of who changed what over the last two years.

**With a central database.** You search the production database, every backup, the logs, the search index and the copies your agents made, and you write down where you looked. Deleting history conflicts with keeping an audit trail. Because of the backups, you can claim the data is deleted, but you cannot prove it.

**With NoCentralDB.** The schema marks personal fields and ties each one to a person. The SDK encrypts each person's fields with that person's own key. An erasure is a work order: one member performs it, a second account signs it off, and the key is destroyed. Without the key, those fields are unreadable in every archive, in every old snapshot and on every device that has deleted the key; a device that never comes back online keeps it, and the list below names every limit ([RFC 0003, "Shredding a person's key"](rfc/0003-keys-and-cryptography.md#shredding-a-persons-key)). Nobody has to find the copies. The signatures stay valid, so the audit trail still shows who changed what, when, and who signed off the erasure.

**In your app.**

```ts
export const model = nc.model({
  customers: nc.collection({
    name: nc.string().personal({ person: "self" }),   // encrypted under the person's key
    email: nc.string().personal({ person: "self" }),
    plan: nc.oneOf(["free", "team"]),                  // not personal, stays readable
  }),
});

await db.workOrders.erase({ person: customerId });  // needs a second account's sign-off
```

**What it does not cover.**

- Devices that never come back online keep their keys and their data.
- A modified client can ignore the erasure and keep the key ([RFC 0003, "Shredding a person's key"](rfc/0003-keys-and-cryptography.md#shredding-a-persons-key)).
- Backups of the relay keep the wrapped key until they expire. The relay cannot open it, but a member of the space who obtains a backup taken before the erasure can ([RFC 0003, "Shredding a person's key"](rfc/0003-keys-and-cryptography.md#shredding-a-persons-key)).
- Copies made outside the database, such as exports and screenshots, stay readable.
- The database does not find personal data in unmarked free text. Keeping names out of free text is the app's job: it stores a reference to the person instead of the name and resolves the reference when it displays the text, so that after an erasure the text shows a placeholder; a name typed or pasted as plain text stays ([RFC 0005, "Personal data in free text"](rfc/0005-schema-and-model-lock.md#personal-data-in-free-text)).
- Removing a member is not erasure: it blocks the member's access from then on, while the data the member wrote and the personal fields about them stay ([RFC 0005, "About whom, not by whom"](rfc/0005-schema-and-model-lock.md#about-whom-not-by-whom)).
- The key is destroyed only once the erasure is stable, when every admin device of the space has written a change after seeing it. Every view shows the fields as erased at once, but an admin device that stays offline keeps the key alive on the members until it comes back or is demoted; after 7 days the SDK of every admin raises an alarm that names it ([RFC 0008, "Effects that wait until a change is stable"](rfc/0008-membership.md#effects-that-wait-until-a-change-is-stable)).

Your data protection officer decides whether destroying a key counts as erasure in your case. This is a design, not legal advice.

**Tell us.** Would a signed-off work order plus a destroyed key be enough evidence for your data protection officer, and what else would they ask for?

### 2. A breach of the relay leaks nothing readable

**The situation.** Someone copies the relay's entire database. The server was compromised, a backup was stolen, or an operator looked where they should not. In the video, you see the dump, and every record in it is ciphertext.

**With a central database.** The server reads the data to filter it, index it and enforce rights on it. Row-level rights protect users from each other. They do not protect users from the server, its operators, its backups or the next breach. A breach of the server exposes everything it stores.

**With NoCentralDB.** The relay appends signed, encrypted changes to each space's log and hands them out. It checks signatures and membership. It holds no keys and folds nothing. Each member decrypts and folds its own copy. A copy of the relay's database therefore contains no readable content.

**In your app.** You write nothing. The SDK encrypts every change before it leaves the device, and keys stay on the members' devices.

**What it does not cover.** The relay sees metadata: which key is in which space, when and how much each key writes, the key generations and when they change, the rhythm of work, and who reads which space from where. It can also withhold changes or show members different subsets of a log. [RFC 0002, "What the relay can see, and what it cannot"](rfc/0002-change-and-log-format.md#what-the-relay-can-see-and-what-it-cannot) lists what it learns from the log, and [RFC 0007, "What the relay sees"](rfc/0007-relay.md#what-the-relay-sees) adds what it learns from its protocol, and what the server it runs on sees. Encryption also does not protect a device from its owner. A process that runs as you can read what you can read.

**Tell us.** Would "the relay sees only ciphertext and this metadata" pass your security review? Which piece of metadata would worry you?

### 3. An outside agent limited to one space

**The situation.** A contractor, or another company's AI agent, should work on one project and nothing else. When the engagement ends, it should lose access.

**With a central database.** The agent's access depends on row-level security policies and on trust in whoever runs the server. One wrong policy, and the agent sees every customer. To remove access, you have to get every policy right again.

**With NoCentralDB.** The agent becomes a member of exactly one space, with its own key and role. It cannot decrypt any other space, because it never received those keys. The agent signs every write with its own key, so everyone sees what it changed. When the engagement ends, you remove the agent from the space, and the space's key rotates. The relay sends the agent no new changes after that. A member that has not yet seen the removal, because it is offline, may still write under the old key generation until it catches up, and the agent could read those changes if it got hold of them ([RFC 0003, "Removing a member and rotating the space key"](rfc/0003-keys-and-cryptography.md#removing-a-member-and-rotating-the-space-key)).

**In your app.**

```ts
const project = await db.spaces.get(projectId);
await project.members.add(invitation, { role: "editor" });  // an invitation the agent accepted

// when the engagement ends
await project.members.remove(memberId);  // rotates the space key
```

You add the agent with an invitation it accepted and remove it by its member id, so your app never handles key material ([RFC 0003, "How the SDK hides keys"](rfc/0003-keys-and-cryptography.md#how-the-sdk-hides-keys)).

**What it does not cover.** Removing a member does not delete what it already received. Its copy keeps the data it had before the removal. Rights apply per space, so a part of a project the agent must not see needs its own space.

**Tell us.** In your app, would one space per project or per shared document work? Where would it break?

### 4. A thousand agent queries a minute, reads at no cost

**The situation.** An agent plans its work in a loop. It reads the ready tasks with their blockers, ranks them, takes the next page and asks again.

**With a central database.** Every query is a round trip. Each one adds latency, counts against a rate limit, loads a server your users also depend on, and costs money. Agents send far more queries than a typical user, and most central databases were not sized for that load.

**With NoCentralDB.** The agent queries its own local copy. Counts, blockers and rankings are computed fields and views, and the database updates them as changes arrive. A page of 50 ready tasks is one range lookup, and the results arrive in rank order. No query crosses the network, so no rate limit or per-query cost applies.

**In your app.**

```ts
const page = await db.find(model.tasks.views.ready.take(50));                     // first 50, ranked
const more = await db.find(model.tasks.views.ready.take(50).after(page.cursor));  // next 50
```

**What it does not cover.** Reads are free; writes are not. Every change an agent writes goes through the relay, which delivers it to every other member of the space, so the relay's cost grows with the writes times the members each write reaches. For 1,000 agents that each write one change a minute, RFC 0007 estimates about 800 USD a month on AWS ([RFC 0007, "What it costs"](rfc/0007-relay.md#what-it-costs)). Queries run only through declared indexes and views. NoCentralDB has no queries on the server and no analytics across all users' data. For those, use a central database.

**Tell us.** Which derived value in your app would not fit our planned computed-field functions (follow, count, any, min, max, reach, cycle)?

### 5. The relay goes down, and everyone keeps working

**The situation.** The relay is unreachable for an hour. Or you move it from SQLite on a small server to DynamoDB in your own cloud account.

**With a central database.** An outage stops the app. Nobody can read or write. A move needs a migration and a maintenance window.

**With NoCentralDB.** Every member keeps reading and writing its full local copy during the outage. Members that can reach each other directly can keep exchanging changes, with the same messages they use with the relay. We call this island mode. When the relay is back, every copy converges to the same state, in whatever order the changes arrive. The relay stores only an append-only log of encrypted changes, which makes it the easiest part of the system to replace.

**In your app.** You write nothing. Reads stay on the device, and the SDK queues writes until the relay answers.

**What it does not cover.** Changes made during the outage reach the other members only when a connection exists again. How members on the same network find each other is not designed yet: RFC 0007 defines the messages two members exchange once they are connected, but not how they find each other, which needs the peer RFC ([RFC 0007, "Island mode"](rfc/0007-relay.md#island-mode)). Until it exists, two members on one network have no designed way to connect to each other. NoCentralDB cannot enforce one global truth at write time, such as unique names or a transaction across all members. We have not yet designed how a move between relay backends works. Your answer below decides it.

**Tell us.** Would you rather run the relay yourself on Postgres, run it serverless in your own cloud account, or use a hosted relay that can only ever see ciphertext?

---

## Part 2. What you keep

### 6. Writing together through a tunnel

**The situation.** Two people in two cities edit the same document. Each sees the other's typing after a short delay. Then one of them is on a train that enters a tunnel. Both keep typing, and their changes stop reaching each other. When the train leaves the tunnel, each screen shows the other person's new paragraphs, and no text is lost.

**How NoCentralDB handles it.** Long text is a shared document, built on Yjs, with Yrs in the Rust core. Each person edits their own local copy, so typing never waits for the network. When the two reconnect, both copies converge without conflicts. Agents can edit too. They read the document as Markdown and change it with anchored edits that quote the text they replace, so a person typing elsewhere in the document loses nothing.

**In your app.** NoCentralDB ships a rich-text field type that works with TipTap:

```ts
export const model = nc.model({
  proposals: nc.collection({
    title: nc.string(),
    body: nc.richText(),   // a shared document, for TipTap
  }),
});

const doc = await db.get(model.proposals, proposalId);
const editor = new Editor({ extensions: [StarterKit, nc.tiptap(doc.body)] });
```

Rich text is one of several field types. You can also define your own field types, each with its own merge rules, and use them in your schema next to the built-in types.

**Tell us.** Which editor or collaborative data type would you want to plug in, and what would it need from us?

### 7. Subscribe instead of poll

**The situation.** A user gets a new chat message, and the app shows a notification at once. Nothing polls.

**How NoCentralDB handles it.** Every member already receives the changes of the spaces it belongs to. A subscription filters that stream. The app declares which changes it cares about, and NoCentralDB calls it when one arrives.

**In your app.**

```ts
db.subscribe(model.messages, { where: { to: me, read: false } }, (message) => {
  notify(`New message from ${message.from.name}`);
});
```

**In the cloud.** The same mechanism starts agents only when something relevant happens. An AWS plugin sends a space's change stream to EventBridge. A function decrypts each change and compares it with the declared subscriptions. The plugin invokes the agent only on a match. Every part is serverless, so nothing polls and nothing costs money while no changes arrive. The decrypting function is a regular member of the space, with its own key, and it appears in the member list like every other member. NoCentralDB has no hidden "server-readable" mode.

**Tell us.** Would a cloud function that is a visible member of the space, with its own key, pass your review? If not, what would you need?

### 8. Streaming as a data type

**The situation.** A user chats with an agent. The answer appears word by word while the agent writes it.

**How NoCentralDB handles it.** The answer is a field of a streaming type. The agent appends chunks to it, and each chunk travels over the same encrypted change stream as every other change. The app needs no separate socket, no second channel to secure and no second store for the conversation. When the agent closes the stream, the field holds the complete answer like any other value.

**In your app**, with TanStack AI:

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

**Tell us.** Should every chunk stay in the log, or should NoCentralDB compact a finished stream into its final text? What would your app need?

---

## How to tell us

Post in the [RFC discussions](https://github.com/nocentraldb/nocentraldb/discussions) on GitHub. Describe your app, your situation and where the design would break for you. Reports of problems help us more than feature requests. We read every reply, answer within a week, and publish a summary of the changes in the next draft of the RFC.
