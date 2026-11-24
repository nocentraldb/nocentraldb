# RFC 0017: The ncdb MCP server, an agent's own member serving the tools an app declares

REQUEST FOR COMMENTS · NOVEMBER 2026 · Ticket NOC-485

This RFC designs `ncdb-mcp`, a server that speaks the Model Context Protocol (MCP) and lets an agent that NoCentralDB's SDKs cannot reach, such as a person's own Claude Code or Codex on their subscription, read and write their spaces. The server is not a gateway to someone else's data: it is itself the agent's member, with its own key and its own full copy, and its tools see only that member's spaces. Its tools are the ones the app declares in its model, such as "open tasks" or "close a task", so one generic binary serves every app with that app's own tools ([The problem](#the-problem)).

It builds on the Rust SDK of [RFC 0009](0009-sdk-api.md) and on the agents and pairing of [RFC 0010](0010-accounts.md); [How this RFC relates to the others](#how-this-rfc-relates-to-the-others) names the rest.

**How to give feedback.** Start a discussion in the category [RFC feedback](https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback), naming RFC 0017 and the section you mean, or open a pull request against this file to change its text. [Giving feedback](README.md#giving-feedback) says how, and what happens to your point.

**Status.** Decided, 2026-11-23. Nothing of it is built, and its build is not scheduled yet.

**At 0.1.** Not built. Release 0.1 has no MCP server; an agent reaches its data only through the Rust SDK or the TypeScript SDK in a browser.

**Out of scope.** A server reachable from the internet, as a custom connector in claude.ai needs, which Anthropic's cloud calls from every Claude app, Claude Desktop included ([custom connectors](https://support.claude.com/en/articles/11175166-get-started-with-custom-connectors-using-remote-mcp)): that is an agent running in the cloud, which joins as a member of its own and is designed with attested agent members (NOC-482). Creating spaces, adding or removing members, erasures, moving objects and ad-hoc spaces for sub-agents as tools ([What the tools leave out](#what-the-tools-leave-out)). The Node.js and Python SDKs, which are planned for later and do not depend on this server.

## Summary

| Question | Answer |
| --- | --- |
| What is it? | A native Rust binary, `ncdb-mcp`, built on the Rust SDK. One data directory is one database and one member of kind agent, with its key in the platform keystore or a key file, and its own full copy. It shows in every space's member list like any agent. |
| How does it become a member? | A person runs `ncdb-mcp init` and `ncdb-mcp pair` once, pairs it with their account as an agent from their app, and adds it to the spaces it should see. No tool can pair it, add it to a space or widen its role. |
| How do clients reach it? | Over stdio, which Claude Code, Codex and Claude Desktop start themselves, and over Streamable HTTP on the loopback interface, with a bearer token and no browser origin. |
| Which process holds the copy? | Exactly one process per data directory, the **host**. `ncdb-mcp serve` is a host that runs until it is stopped, and an app such as a Tauri app can start it. `ncdb-mcp stdio` forwards to a running host, or becomes the host itself and runs until its last client has gone. |
| How current is the copy? | The host syncs in the background for as long as it runs. Reads never wait for the network; every tool result says when the copy was last known to be current, per space. A read may ask to wait for the sync, up to 20 s. |
| Which tools? | The tools the app declares in its model: finds over its declared indexes, gets of one object, and actions, each a fixed set of creates, updates and deletes made from the tool's parameters, all with the app's names and descriptions. `ncdb model build` checks them and writes them into the lock. `list_spaces` is always there. Seven generic tools, such as `find` and `write` over any collection, are listed only with the developer flag `--generic-tools`. Results are structured JSON with a schema. |
| What does the model see? | Data: the fields the app's tools name, of the objects it reads. Never a key, an invitation, a pairing answer or an SDK cursor. Everything a tool returns goes to the model's provider. |
| How does it ship? | As a binary per platform on each GitHub release, built and attested by the release workflow, and as the crate `ncdb-mcp` on crates.io for `cargo install`. An app can bundle the binary as a sidecar. |

## The problem

GENESIS.md asks for data close to the agent, and for MCP not to be the only way in. The SDKs do the first for every agent whose loop a developer writes: the loop links the Rust SDK, or later the Node.js SDK, and reads its own copy. Two kinds of agents cannot link an SDK:

- **A person's own Claude Code or Codex, on their own subscription.** These are another vendor's processes. A third-party app may not sign the person in to the subscription or route its requests through it ([Claude Code, "Legal and compliance"](https://code.claude.com/docs/en/legal-and-compliance)); the person runs the unmodified client and signs in there. What NoCentralDB can be in that picture is a tool the client calls.
- **Agent frameworks in Python or TypeScript** that a developer runs before NoCentralDB has an SDK in that language. They speak MCP already.

Without a server of our own, the only path for both would be a remote MCP service that holds the data for them. That service would read content it does not need to read, and would be the hidden reader GENESIS.md rules out. A local server that is itself the member keeps both rules: the copy sits next to the agent, and whatever reads the content is a member with its own key, in the member list.

**Which tools such a server offers is the app's to say.** A server NoCentralDB ships knows only the lock: on its own it could offer generic tools, such as a find over any index and a write into any collection, and leave the model to work out the app's schema and its rules. The app knows its domain, and a model uses "close this task" or "the open tasks of a project" far more reliably than a generic write it has to compose. What only NoCentralDB can do, and every app would otherwise build again, is the rest: the agent as its own member, its key, pairing, the full copy and its sync, the host, the loopback transport, and the rule that no key reaches the model. So the app declares its tools in its model, `ncdb model build` checks them, and the one binary serves exactly those ([Declared tools](#declared-tools)).

## Terms

| Term | Meaning in this RFC |
| --- | --- |
| Space, member, agent, change, object, account, pairing | As in [RFC 0009](0009-sdk-api.md), [RFC 0010](0010-accounts.md) and the [glossary](../glossary.md). |
| Client | The program that calls the server's tools: Claude Code, Codex, Claude Desktop, an agent framework. |
| Model | In this RFC alone, also the language model behind a client, called "the model" where it cannot be read as the lock's model. Its provider is the company that runs it. |
| Data directory | The directory the server's database lives in, as the Rust SDK's `path` names it. One data directory is one member. |
| Host | The one process that has opened a data directory's database and syncs it. |
| Tool, tool result | MCP's names for a call the client makes and what it gets back. |
| Declared tool | A tool the app declares in its schema module: a find, a get or an action. |
| Action | A declared tool that writes: a fixed list of creates, updates and deletes, filled from the tool's parameters, written as one change. |
| Generic tools | The seven tools over any collection that the server lists only with `--generic-tools`. |

## What the server is

`ncdb-mcp` is a Rust binary on the Rust SDK, in a crate of its own, `crates/ncdb-mcp`. It opens one database with `Db::open(OpenOptions::new(lock).path(dir).agent())`, so its member is certified as an agent ([RFC 0009, "The Rust SDK"](0009-sdk-api.md#the-rust-sdk)), and serves MCP with rmcp, the official Rust SDK for MCP, at the protocol's current version, 2026-07-28, and its predecessor 2025-11-25, which rmcp both speaks.

- **One data directory, one member, one host.** Everything the server does is that member's doing: its writes carry that member's signature, and the member list shows it as an agent of the account that paired it.
- **The lock is read at run time.** The binary is the same for every app: it takes the path of an app's `model.lock.json`, lists the tools the lock declares, and reads and writes fields by name, as the Rust SDK does for every lock ([RFC 0009, "The Rust SDK"](0009-sdk-api.md#the-rust-sdk)). It runs no code of the app. A newer lock is checked on the device as `Db::open` checks it, and a lock the database refuses stops the server with `modelRefused`.
- **Keys stay in the SDK.** The member key lives in the platform keystore, one entry per database, or in a key file the person names with `--key-file` for a machine without a keystore ([RFC 0009, "Open and close"](0009-sdk-api.md#open-and-close)). The server never reads a key; no tool returns one.

### Options not taken

- **A subcommand of `ncdb`**, as `ncdb mcp`. `ncdb` builds and checks the lock, and ships through npm as a WebAssembly module inside `@nocentraldb/sdk` (NOC-443). The server needs the platform keystore, SQLite on disk, the relay connection and a long-running process, which a WebAssembly module cannot have, and would make every developer who only builds a lock download all of it.
- **A server in TypeScript on the browser's core.** It would need a Node.js SDK, which is planned for later, and would still be a long-running process with a key; Rust is the SDK that runs there today.
- **A crate each app builds its own server on.** NoCentralDB would ship the host, the membership, the sync and the transports as a Rust library, and each app would write its own MCP server with its own tools. The tools would be as good as with declared tools, but every app would need Rust and a server of its own to build, ship and sign, and the identity rules below would be in every app's hands. An app in TypeScript could not do it before a Node.js SDK exists.

## Becoming a member

The server becomes a member the way any agent does ([RFC 0010, "Adding a device or an agent to an account"](0010-accounts.md#adding-a-device-or-an-agent-to-an-account)). A person does it once, in a terminal or through their app:

```sh
ncdb-mcp init --data ~/.ncdb/agents/claude --lock ./model.lock.json --relay wss://relay.example.com
# prints the agent's invitation, ncdb-invite-1:...
# the person pairs it from a device of their account, as kind "agent", and gets a pairing answer
ncdb-mcp pair --data ~/.ncdb/agents/claude ncdb-pair-1:...
```

1. **`init`** creates the data directory, opens the database for pairing as an agent and prints its invitation. It stores the lock's path and the relay in the data directory's settings, so later commands name only `--data`.
2. **The person pairs it** from a device of their account with `db.account.pair({ kind: "agent", invitation })`, after comparing the fingerprint, as RFC 0010 asks. The answer gives the agent its certificate and no account key: an agent never holds one.
3. **`pair`** completes the pairing with the answer.
4. **An admin adds the agent to each space** it should see, with the role it should have, through the app, as `members.add` adds any agent. An agent added without a role joins as a writer ([RFC 0010, "What a member certificate means for RFC 0008"](0010-accounts.md#what-a-member-certificate-means-for-rfc-0008)); a person who wants it to read only adds it as a reader.

Removing it is removing any agent: `db.account.removeMember` from a device of the account, or `members.remove` per space. The removed copy keeps what it held, as every removed member does ([GENESIS.md, "What we promise"](../../factory/GENESIS.md#what-it-must-never-do)).

No tool pairs, adds, removes or changes roles. The person decides what the agent can read, outside the model's reach, so data the agent reads cannot talk it into reading more.

### Options not taken

- **Pairing as a tool**, so the agent sets itself up from a chat. The invitation and the answer would pass through the model's provider, and the model could be asked by data to add itself elsewhere. Pairing is rare, and a terminal command is enough.
- **The server drawing an account of its own**, as an unpaired database does. The agent would then answer for nobody: its certificate would name an account no person holds, and the member list could not say whose agent it is.

## Transports

The server speaks MCP over two transports.

- **stdio.** The client starts `ncdb-mcp stdio --data <dir>` and talks to it over standard input and output. Claude Code, Codex and Claude Desktop start local servers this way. Over stdio the server takes no credential: whoever can start the process can open the database anyway.
- **Streamable HTTP on the loopback interface.** `ncdb-mcp serve --data <dir>` listens on `127.0.0.1` and, where available, `::1`, on the port in the data directory's settings, 47017 by default. Clients that call HTTP servers, and every framework that speaks Streamable HTTP, reach it there.

**Authentication on the loopback interface.** A port on the loopback interface is open to every process on the machine, every user's included, and to web pages through DNS rebinding. So the host:

1. listens only on loopback addresses, and refuses to start on any other address. There is no option to listen elsewhere;
2. requires `Authorization: Bearer <token>` on every request. The token is 32 random bytes, drawn at `init`, kept in the data directory in a file only the user who runs the server may read (mode 0600, or an ACL for that user alone on Windows), and compared in constant time. `ncdb-mcp token` prints it, and `ncdb-mcp token --rotate` draws a new one;
3. answers 403 to any request that carries an `Origin` header, and to any whose `Host` is not a loopback address or `localhost` with the host's port. MCP requires a server to check `Origin` against DNS rebinding; clients that are not browsers send none, and a web page always does, so no page can reach the host.

`ncdb-mcp stdio` reads the token from the file itself, so a client that starts it needs none: this is how Claude Code and Codex are set up. A client that calls the host over HTTP sends the token from an environment variable where it can, such as Codex's `bearer_token_env_var`, so the token never sits in a configuration file that is shared or committed. The token keeps out other users of the machine and web pages. It does not keep out a process running as the same user, which can read the token file, and the database too ([RFC 0006, "What the view stores in plaintext"](0006-local-view.md#what-the-view-stores-in-plaintext)): the device is not a boundary against the user's own processes.

### Options not taken

- **MCP's OAuth 2.1 authorization.** It needs an authorization server, and is meant for servers on the network. For a host on the loopback interface, owned by the user, a static bearer token gives the same protection with nothing to run, and every client that speaks HTTP can send one.
- **No authentication on the loopback interface.** Any other user's process, and any web page through DNS rebinding, could then read every space the agent belongs to.
- **A Unix domain socket or a named pipe** between the stdio command and the host. File permissions would replace the token, but clients and frameworks that speak HTTP could not use it, and Windows would need a second path. One transport keeps one path to test.
- **Listening on another address,** for an agent on another machine. That agent should be a member of its own, with its own copy; a host on the network would let it read without being one.

## Which process holds the copy

A database is opened by one process at a time. The host takes an exclusive lock on a file in the data directory, which the operating system releases when the process ends, also when it crashes, and writes its port and process id next to it.

- **`ncdb-mcp serve`** is a host that runs until it is stopped. An app, such as a Tauri app, can start it as a sidecar when it starts, or the person can run it as a service of their operating system. It syncs all the time it runs, so the copy is current when a client calls.
- **`ncdb-mcp stdio`**, which a client starts, first looks for a host of its data directory. When one answers, it forwards every message between the client and the host and holds nothing itself. When none runs, it takes the lock and becomes the host: it opens the database, syncs in the background and listens on the loopback interface like `serve`, so the next client's `stdio` forwards to it. It runs until its own client and every forwarding client have gone, then closes the database and exits. Two `stdio` commands that start at once race for the lock, and the one that loses forwards to the one that wins.
- **`ncdb-mcp serve` finds a host running** for its data directory: it exits with an error that names the host's process id. The person stops that host, and the next client's `stdio` forwards to `serve`.

So two terminals with Claude Code share one copy and one member, as two tabs of one app share one database ([RFC 0012](0012-multi-tab-browser-storage.md)), and no copy is opened twice.

### Options not taken

- **One process per client session, each with its own copy.** Two sessions would keep two copies of the same member's database, opened twice, with two relay connections, and two processes would write under one member key, each counting its own sequence, so the member would write two changes under one sequence and show as equivocating ([RFC 0008, "The cutoff"](0008-membership.md#the-cutoff)).
- **Only `serve`.** A person without an app that starts it would have to start a service before their first call. `stdio` alone gives a working server with nothing set up beforehand.
- **`stdio` starting `serve` in the background** when no host runs. The person would find a process running after their session ended that they never started.

## How current the copy is

The host syncs as the Rust SDK does: one connection to the relay, pulling and pushing every space, reconnecting with backoff ([RFC 0009, "Sync and status"](0009-sdk-api.md#sync-and-status)). Reads read the local view and never wait for the network ([RFC 0006, "Reads never touch the network"](0006-local-view.md#reads-never-touch-the-network)). A host started by `serve` is current within seconds of a change; a host started by `stdio` starts cold, from the copy it held when its last session ended.

**Every tool result says how current the copy is.** Each result that reads or writes a space carries `sync`:

```json
{ "sync": { "connection": "connected", "current": true, "syncedAt": "2026-11-23T09:14:02.118Z" } }
```

- `connection` is the SDK's `connection`: `connected`, `connecting`, `offline`, `paused` or `none`.
- `current` is true while the host holds everything the relay has for the space: connected, the first pull of this run complete, and no position the host has heard of but not folded (`initial_sync_complete` and `behind == 0` in the space's `SpaceStatus`).
- `syncedAt` is the last moment `current` was true in this host's run, as a point in time in UTC; it is now while `current` is true. It is `null` until the space is current for the first time in this run, which tells the model that the copy may be as old as the last session. A result over several spaces, such as `find`, carries `sync` per space.

**Waiting for the sync.** Every tool that reads, declared or generic, takes `waitForSync`, a number of milliseconds from 0 to 20000, 0 by default. With it, the host waits until the space is `current` or the time is up, and then reads; the result's `sync` says which. The wait is the host's, not the read's: the read still reads only the local view. 20 s keeps a call well inside Codex's limit of 60 s per tool call.

**What the clients' limits mean.** Codex gives a server 10 s to start and 60 s per tool call by default (`startup_timeout_sec`, `tool_timeout_sec`); Claude Code gives it 30 s to start (`MCP_TIMEOUT`) and about 28 hours per call ([Codex, MCP](https://learn.chatgpt.com/docs/extend/mcp?surface=cli); [Claude Code, MCP](https://code.claude.com/docs/en/mcp)). So the host answers MCP's `initialize` and `tools/list` at once, from the lock alone, and opens the database meanwhile: opening may wait for the keystore, which can ask the person, or start a rebuild ([RFC 0009, "Open and close"](0009-sdk-api.md#open-and-close)). A tool call that arrives before the database is open waits for it, within the call's own deadline. Every call has a deadline of 30 s in the host, and a call that runs over returns the error `timeout`; a write is one change, committed whole or not at all, so no call leaves half a write behind.

**Push.** MCP lets a server tell a client that something changed, but the clients support it only in parts: Claude Code refreshes its tool list on `list_changed` and documents no subscriptions to resources, and its channels, which push messages into a session, are a research preview. A model does not act on a notice it is not asked about. The host sends `notifications/tools/list_changed` only when its tools change, after a newer lock. It does not tell the client that data changed; the model reads again, and `sync` says how fresh the result is.

### Options not taken

- **Syncing on every tool call.** Every call would wait for the relay, after a long pause for much of a 60 s limit, and a call would fail whenever the relay is out of reach, though island mode lets every other member read on.
- **Keeping `syncedAt` across runs** in the data directory. It would say when the last run was current, which the model cannot tell apart from a current copy unless it reads the date; `null` says plainly that this run has not caught up yet.
- **MCP resources with subscriptions**, so the client hears of new data. No client this RFC is for documents subscriptions a model acts on, and a resource per object or per query would duplicate the tools.

## Declared tools

The app declares its agent tools in its schema module, next to its model. `ncdb model build` checks them against the model and writes them into the lock, and `ncdb-mcp` serves them. A declared tool is data, not code: the binary never runs the app's TypeScript, so a tool can only read through a declared index or by id, and write a fixed list of operations filled from its parameters.

### In the schema

```ts
import { nc } from "@nocentraldb/sdk";

export const model = nc.model({ /* the task board of RFC 0005 */ });

export const tools = nc.tools(model, {
  openTasks: nc.tool.find(model.tasks.indexes.byStatusPriority, {
    description: "Open tasks across the person's projects, most urgent first.",
    equal: ["open"],
    fields: ["title", "priority", "assignee", "body"],
  }),
  task: nc.tool.get(model.tasks, {
    description: "One task with its notes.",
    fields: ["title", "status", "priority", "body", "dependsOn"],
  }),
  closeTask: nc.tool.action({
    description: "Mark a task as done.",
    params: { task: nc.param.object(model.tasks).describe("The task to close.") },
    write: (p) => [nc.op.update(p.task, { status: "done" })],
  }),
  addTask: nc.tool.action({
    description: "Add an open task.",
    params: {
      title: nc.string().describe("One line; what has to be done."),
      priority: nc.int().optional(),
      tag: nc.string().optional(),
    },
    write: (p) => [nc.op.create(model.tasks, { title: p.title, priority: p.priority, status: "open", tags: nc.add(p.tag) })],
  }),
  editTaskNotes: nc.tool.action({
    description: "Change part of a task's notes by quoting the text to replace.",
    params: { task: nc.param.object(model.tasks), edits: nc.param.textEdits() },
    write: (p) => [nc.op.update(p.task, { body: p.edits })],
  }),
});
```

- **`nc.tools(model, { name: tool })`** takes the model and the tools by name. The schema module exports it as `tools`, beside `model`; `ncdb model build` reads both. A module without `tools` declares none.
- **A tool's name is its key, as the app writes it,** such as `closeTask`. The model, the client and the logs show what the developer typed, and no two keys can turn into one name. MCP prescribes no case; an app that wants `close_task` writes it so.
- **`nc.tool.find(index, { description, equal, fields, take })`** reads a page of a declared index, as `db.find` does. `equal` fixes some of the index's leading fields to constants; the leading fields the index's `equal` asks for and the tool does not fix become the tool's parameters, typed as their fields. An index with `scope: "space"` adds a `space` parameter. The model passes `take`, up to the tool's `take`, 20 by default and at most 100, and `after`, a cursor handle.
- **`nc.tool.get(collection, { description, fields })`** reads one object, as `space.get` does. Its parameters are `space` and `id`.
- **`fields`** names the fields a find or a get returns, and nothing else is returned. The app chooses what the model sees: a personal field reaches the model only when a tool names it. A rich-text field reads as `{ markdown, version }`, the version an action's text edits name ([RFC 0013, "An agent: anchored edits"](0013-rich-text.md#an-agent-anchored-edits)).
- **`nc.tool.action({ description, params, write })`** writes. `params` takes field functions, `nc.string()`, `nc.int()`, `nc.oneOf([...])`, `nc.date()` and the other basic types of [RFC 0005](0005-schema-and-model-lock.md#field-types), with `.describe(text)` and `.optional()`, and two kinds of its own: `nc.param.object(collection)`, an object of that collection in the action's space, and `nc.param.textEdits()`, quoted edits of a rich-text field. `write` is a function the build calls once with placeholders for the parameters, and it returns the operations: `nc.op.create(collection, values)`, `nc.op.update(object, values)` and `nc.op.delete(object)`. A value is a constant, a parameter, or `nc.add` and `nc.remove` of either, as in [RFC 0009, "Field operations per field type"](0009-sdk-api.md#field-operations-per-field-type). An object created by an earlier operation can be the target or a link value of a later one, as in `space.write`.
- **Every action has a `space` parameter,** and writes into that space only: an action is one `write`, one change in one space, like every write ([RFC 0009, "One write, one change"](0009-sdk-api.md#one-write-one-change)). The model takes the space from an object it read, whose `$space` names it.
- **`description`** is required on every tool and every parameter that is not a field of the model, which carries its own `.describe()`. The model reads only these texts to choose a tool.

### In the lock

`ncdb model build` writes the tools under `tools`, beside `collections`, in the lock's canonical form ([RFC 0005, "Canonical form"](0005-schema-and-model-lock.md#canonical-form)). A lock that declares tools has `format` 2; a lock that declares none stays at format 1. Every SDK reads format 2 and ignores `tools`, which only `ncdb-mcp` reads. An SDK that reads only format 1 refuses a lock with tools openly, as RFC 0005 has a reader do with a format it does not know, rather than ignoring what it cannot read; an app that declares tools therefore needs a current SDK.

```json
{
  "tools": {
    "closeTask": {
      "description": "Mark a task as done.",
      "kind": "action",
      "params": {
        "task": { "collection": "tasks", "description": "The task to close.", "type": "object" }
      },
      "write": [
        { "op": "update", "target": { "param": "task" }, "values": { "status": { "value": "done" } } }
      ]
    },
    "openTasks": {
      "collection": "tasks",
      "description": "Open tasks across the person's projects, most urgent first.",
      "equal": [{ "value": "open" }],
      "fields": ["assignee", "body", "priority", "title"],
      "index": "byStatusPriority",
      "kind": "find",
      "take": 20
    }
  }
}
```

A tool changes no data and never travels in a change, as an index does not. So adding, changing or removing a tool is always accepted by the additive-change rule, and leaves the model version and `model.hash` as they are, as a description does ([RFC 0005, "The additive-change rule"](0005-schema-and-model-lock.md#the-additive-change-rule)). `ncdb model check` compares the bytes of the whole lock, tools included, so a lock whose tools are out of date fails in CI.

### What the build checks

`ncdb model build` refuses a declared tool, naming it and what is wrong, when:

- its name does not match `^[a-z][A-Za-z0-9_]{0,63}$`, two names differ only in case, or it takes the name of a generic tool or of `list_spaces`;
- it has no description, or a parameter that is not a field of the model has none;
- a find names no declared index, fixes more fields than the index's `equal`, fixes one with a constant of the wrong type, or sets `take` above 100;
- a find or a get names a field the collection does not have, a deprecated field, or a computed field;
- an action writes into a deprecated collection or field, writes a value whose type does not fit the field (a parameter's type is checked against the field it fills, as a value of `space.write` is), uses `nc.add` or `nc.remove` on a field that is not a set or many links, fills a rich-text field from anything but `nc.param.textEdits()`, targets an object that is not an object parameter or an earlier create, or declares a parameter it never uses;
- an action has no operation, or more than 50.

The rules live in Rust, in the code `ncdb model build` already runs as WebAssembly (NOC-443), and `ncdb-mcp` runs the same check when it opens a lock: a lock whose tools fail it, such as one edited by hand, stops the server with `toolsRefused`, which names each refused tool.

### How a declared tool runs

- **A find** fills the index's equality values from the constants and the parameters, reads the page with `db.find`, or `space.find` for an index per space, and returns `{ items, cursor, hasMore, sync }`, each item with the tool's `fields`, `$id`, `$space`, and `$conflicts` for those fields.
- **A get** reads the object with `space.get` and returns the tool's `fields` with `$id`, `$space`, `$deleted` and `$conflicts`, or `null`.
- **An action** checks the parameters against their types, opens one `space.write` in the given space, and runs the operations in order: a create with its values, an update of the target with its values, a delete of the target. A parameter left out leaves its field unchanged in an update and unset in a create; `nc.add` and `nc.remove` of a left-out parameter do nothing. An object parameter that names no object of its collection in the space fails with `notFound`, before anything is written. Text edits apply as RFC 0013's anchored edits, at the version the model read. The action returns `{ change, ids, sync }`, the ids of the objects it created by their order.

Every refusal of the core applies as to any write: a member added as a reader is refused with `notAllowed`, and the change is written whole or not at all.

**What the server lists for a tool.** Each declared tool becomes an MCP tool under its name, with its description, an `inputSchema` built from its parameters and their descriptions, and an `outputSchema` built from its fields. Finds and gets carry `readOnlyHint`. An action carries `destructiveHint` when it updates or deletes, and `destructiveHint: false` when it only creates. Every tool carries `openWorldHint: false`.

### Options not taken

- **Only generic tools,** as `find` and `write` over the lock. They work with every app on the first day, but leave the model to learn the app's schema and its rules from `describe_model`, and to compose writes the app never meant. They stay, behind `--generic-tools`, for developers.
- **Tools as code,** a function in the app that the server calls. The binary would have to run the app's TypeScript, or every app would write Rust; a declared tool is data the binary interprets, and the build can check it.
- **A query language in a tool,** such as a filter on any field. A query runs only through a declared index ([GENESIS.md, principle 6](../../factory/GENESIS.md#principles-in-priority-order)); a tool that needs another order declares another index.
- **Returning every field.** The app would not choose what reaches the model's provider; naming the fields keeps personal fields out unless a tool asks for them, and keeps results small.
- **Tools in a file of their own, apart from the schema module.** They name the model's collections, fields and indexes, and the build checks them against the model; written beside it, TypeScript checks the names as the developer types them.
- **A generated file of their own, `model.tools.json`, beside the lock.** The lock's format would stay as it is, but there would be a second generated file to commit and to check, and a second path for `ncdb-mcp` to take and keep in step with the lock, which can drift from it.
- **`tools` in the lock at format 1.** Nothing new to pass around, but every reader of format 1 would have to ignore a top-level key it does not know, which format 1 never promised; each SDK's reader would need checking and new tests, and an old reader would silently skip what it cannot read.
- **One namespace, `nc.agent`,** with `nc.agent.tools`, `nc.agent.find`, `nc.agent.action`, `nc.agent.create` and the rest. Everything for agents would sit in one place, at the cost of longer names, and `nc.agent.create` reads like creating an agent, a mistake the API would carry for good. `nc.tool`, `nc.param` and `nc.op` each say what they hold.
- **Tool names converted to snake case,** `close_task` for `closeTask`, the usual form in MCP. The model and the logs would show a name the developer never typed, and two keys could turn into one name, which the build would then have to refuse.

## The tools

The server lists, for each session:

- **The declared tools** of the lock, as [Declared tools](#declared-tools) describes.
- **`list_spaces`,** always: the member's spaces, with each one's id, the member's role, whether the space is of the lock's model, `sync`, and the counts of its parked and rejected changes and its gaps. It belongs to the member, not to the app, and tells the model where it can act and how fresh each space is.
- **The generic tools,** only with `--generic-tools`, for a developer who builds or tests an app's tools: `describe_model`, `find`, `get`, `inspect`, `write`, `read_rich_text` and `edit_rich_text`, which follow the SDK's calls of the same names over any collection and index of the lock.

| Generic tool | Does | Annotations |
| --- | --- | --- |
| `describe_model` | The lock as the model needs it: each collection with its fields, their types and whether they are personal, and its indexes with the fields they fix and the order they sort by. | read-only |
| `find` | A page of a declared index, as `db.find`, or of one space, as `space.find`: `collection`, `index`, `equal`, `take`, `after`, optionally `space`. | read-only |
| `get` | One object, as `space.get`: `space`, `collection`, `id`. | read-only |
| `inspect` | One object's conflicts, lost removals and link states, as `space.inspect`. | read-only |
| `write` | Creates, updates and deletes in one space, as one `write`: one change. | destructive |
| `read_rich_text` | A rich-text field as Markdown, and its version, as RFC 0013's `readRichText`. | read-only |
| `edit_rich_text` | Quoted edits of a rich-text field at a version, as RFC 0013's anchored edits. | destructive |

The generic tools return every field, personal fields included, and write any field; that is why they wait behind a flag a developer sets. A client shows every tool under the server's name, such as `mcp__ncdb__closeTask` in Claude Code.

MCP's annotations are a hint: MCP asks clients not to rely on them. The member's role and `--read-only` are the limit. **`--read-only`** lists only the tools that read: the declared finds and gets, `list_spaces`, and the generic reads. A member added as a reader is refused every write by the core anyway; the option keeps a writer's actions out of the model's reach for a session that should only read.

### Results

Each result is MCP structured content, a JSON object that matches the tool's `outputSchema`, with the same JSON as text beside it for clients that read only text.

- **An object** reads as RFC 0009's read object in TypeScript, limited to the tool's fields: its fields by name, `$id`, `$space`, `$deleted`, `$conflicts` and `$raw`. A day is `"YYYY-MM-DD"`, a point in time an RFC 3339 string in UTC, a set an array, a link an object id, a reference `{ "space", "id" }`, and an int outside −(2^53 − 1) to 2^53 − 1 a decimal string. An erased field is left out and named in `$erased`.
- **A page** is `{ items, cursor, hasMore, sync }`. `take` is at most 100, fewer than the SDK's 1000, because every item becomes part of the model's context: Claude Code warns at 10,000 tokens of a tool's output and moves output over 25,000 tokens into a file.
- **A cursor** is an opaque handle the host draws and maps to the SDK's cursor in memory, for one hour or until the host ends. An SDK cursor holds plaintext field values and never leaves the device ([RFC 0009, "Queries and indexes"](0009-sdk-api.md#queries-and-indexes)); the handle carries none. A handle that has run out fails with `cursorExpired`, and the model starts from the first page.
- **A write** returns `{ change, ids, sync }`: the change id and the ids of created objects. It resolves when the change is in the local log and folded, as `write` does; `sync.connection` says whether the relay can hear of it yet.

Values a tool takes use the JSON of the results. In the generic `write`, `null` clears a field, and `{ "add": [...] }` and `{ "remove": [...] }` change a set or many links, as in [RFC 0009, "Field operations per field type"](0009-sdk-api.md#field-operations-per-field-type). A rich-text field changes only through text edits, never through a whole value, so the model never writes a whole document ([GENESIS.md, "What it must never do"](../../factory/GENESIS.md#what-it-must-never-do)).

### Errors

A tool that fails returns a result with `isError` and `{ "code", "message" }`. The code is the SDK's error code as TypeScript names it, such as `notAllowed`, `invalidValue`, `notFound` or `anchorNotFound`, plus `timeout` and `cursorExpired` of this RFC. The message says what to do next, such as "quote more of the text", because the model reads it. An error never carries a key, and carries content only where the call itself sent it.

### What the tools leave out

- **Creating spaces, adding and removing members, changing roles, pairing.** These change who can read, which GENESIS.md asks the SDK to make plain to the developer; the person does them through their app. No declared tool can do them either: an action writes only objects of the model.
- **Erasures and moves.** An erasure needs a sign-off by a second account, and a move changes which members see an object ([RFC 0016](0016-moving-an-object.md)). Both are rare, carry notices the person should read, and come later if agents need them.
- **Ad-hoc spaces for sub-agents.** They need a sub-agent that opens a database of its own, which a client's sub-agents do not; they come when a client can start one.
- **Subscriptions.** A model reads when it acts; the client has no channel to wake it on a change.

### Options not taken

- **One generic tool,** `call`, taking an SDK call by name. It saves tool definitions but gives the model no schema per call, and annotations would not tell a read from a write.
- **A tool per collection,** such as `find_tasks`, made from the lock without the app's say. The tools would follow the lock's structure, not the app's tasks, and would carry no description of what they are for.
- **`list_spaces` behind `--generic-tools` too,** so that only the app's tools are listed without the flag. The model would learn which spaces it may act in, and how fresh each is, only from the `$space` and `sync` of what it reads; how current a space is belongs to the member, not to the app, and an app could declare no tool that gives it.
- **The flag named `--dev`.** Short, but it says nothing of what it turns on, and may later be wanted for something else; `--generic-tools` says what it adds.
- **The generic tools always listed** beside the declared ones. The model would choose between `closeTask` and a generic `write` that can do the same and more, and personal fields the app keeps out of its tools would be one call away.
- **Text only, without structured content.** Clients and frameworks that check results against a schema could not, and a model gets nothing it does not get from the text beside it.
- **MCP resources for objects.** The clients' support for resources differs (Claude Code offers them as @-mentions, Codex as tools of its own), and a resource names a fixed object; the tools serve every client the same way.

## Identity rules

The server keeps GENESIS.md's rules on members and agents by construction:

- **One host per member.** A data directory is one member, and one host holds it. The server has no notion of tenants or users: it serves the one member whose database it opened.
- **One member per agent.** Each agent that should have an identity of its own gets a data directory of its own: Claude Code and Codex, for example, run two hosts with two members, on two ports, each with its own token, and each is added to its spaces on its own. Several sessions of one agent share its host and its member, as several tabs of one app share a database. A host is never shared by agents that should be told apart in the member list.
- **HTTP only on the loopback interface, behind the token.** The host cannot be made to listen elsewhere ([Transports](#transports)).
- **The tools see only the member's spaces.** The host holds no copy but its member's, so there is nothing else to see; a space the member was removed from reads as the SDK reads it, with role `removed`.
- **The model sees data, never keys.** No tool returns a key, an invitation, a pairing answer, a token or an SDK cursor. The host logs no content, no key and no token, only counts, error codes and timings, as the SDK's logs do ([RFC 0003, "Keys never reach logs, errors or telemetry"](0003-keys-and-cryptography.md#keys-never-reach-logs-errors-or-telemetry)).

### Options not taken

- **One host for several agents, each with a token of its own.** The host would tell its callers apart, but every write would still carry one member's signature, and the member list would show one agent where there are several: the hidden reader GENESIS.md rules out.
- **A member per session.** Each Claude Code session would need its own pairing and its own adds to every space, and the member list would fill with members that each lived for an hour.

## What this does not do

- **It does not keep data from the model's provider.** Everything a tool returns goes into the model's context, and so to the company that runs the model, under the terms of the person's subscription or API contract. The relay still reads nothing; the model's provider reads what the agent reads. A person adds the agent to a space knowing this, and the member list shows that the agent is there.
- **It does not stop prompt injection.** An object's text can hold instructions, and a model may follow them: write into a space, or pass what it read to another tool, such as one that fetches a web page. MCP does not cause this; every agent that reads data and can act has it. The server limits what is at stake, not whether it happens: the agent sees only its spaces and only the fields the app's tools name, its actions write only what the app declared, `--read-only` takes them away, a reader role makes the core refuse them, and the client may ask the person before each write. Small spaces keep what one agent can leak small.
- **It does not judge the app's tools.** The build checks that a declared tool fits the model, not that it is wise: an action that deletes whatever object the model names does exactly that.
- **It does not replace the SDKs.** An app's own agent loop links the Rust SDK and reads its copy without a server between. MCP is one more way in, not the only one.
- **It does not serve claude.ai's custom connectors.** Anthropic's cloud calls a custom connector over the internet, from every Claude app, and the host listens only on the loopback interface. Claude Desktop reaches it as a local server, as Claude Code does. An agent in the cloud is a member of its own, designed with attested agent members (NOC-482).
- **It does not keep a process from reading the database.** A process running as the same user can read the data directory and the token, as for every app on the device.
- **It does not make an offline copy current.** `sync` says how old the copy is; it does not fetch what the relay cannot deliver.

## How it ships

The server is a native binary that runs for a long time and holds a key in the platform keystore, so it ships as a native binary, not as a script or a WebAssembly module.

- **On each GitHub release, a binary per platform:** macOS on Apple silicon and Intel, Linux on x86-64 and ARM64, statically linked against musl, and Windows on x86-64, each with a SHA-256 checksum and a build provenance attestation from the release workflow, so a person can check that the binary came from this repository's workflow.
- **On crates.io, the crate `ncdb-mcp`,** published by the registry job that publishes `nocentraldb`, so `cargo install ncdb-mcp` builds it from source.
- **Bundled by an app.** A Tauri app adds the binary as a sidecar and starts `ncdb-mcp serve` with the app's data directory for the agent. The app then guides the person through pairing and adding the agent.
- **Signed binaries.** The macOS binaries are signed with a Developer ID and notarized, and the Windows binary signed, so the keystore and the operating system's checks accept them without warnings. The macOS signing uses an Apple developer membership that is already paid for; the Windows certificate and the macOS and Windows runners for release builds are paid.

Alphas carry no binaries; betas and releases do, as they carry the crates.

### Options not taken

- **npm packages with a binary per platform**, as `npx @nocentraldb/mcp`. Many MCP servers start this way, and a JavaScript developer has npx at hand. It needs one package per platform, each with its own placeholder, trusted publisher and approval per release, which NOC-443 turned down for `ncdb`; and it would download and start a binary through npm on every session. It can be added later on top of the release binaries.
- **A Claude Desktop extension (`.mcpb`).** It installs a local server in Claude Desktop with one click. It is a thin package around the release binaries and can come later; Claude Code and Codex, the clients this RFC is for, do not use it.
- **Homebrew, winget and Linux packages.** Each is a channel to keep up; the release binaries and `cargo install` come first.
- **Unsigned binaries.** macOS would warn on every download, and the keystore may ask the person again for each new build.

## Agreement with GENESIS.md

| GENESIS.md | How this RFC keeps it |
| --- | --- |
| Agents are fast when data is close; avoid MCP as the only way in | The server reads its own full local copy; the SDKs stay the first way in. |
| Never add a hidden reader | The server is a member of kind agent, in the member list, paired by a person and added per space. |
| Never give an agent more than its spaces | The host holds only its member's copy, and no tool widens it. |
| The relay never reads content | Unchanged: the server syncs as any member does. |
| Never write content, keys or personal fields into logs | The host logs counts, codes and timings only; no tool returns a key or a token. |
| Never overwrite more than you change | Rich text changes only through quoted edits, in a declared action or the generic `edit_rich_text`; no tool writes a whole document. |
| Apps read from a computed result, through declared indexes | A declared find reads only through a declared index; no tool scans. |
| Never put the network on the read path | Reads read the local view; `waitForSync` waits for the background sync, and reads only the view. |
| Say plainly what it does not do | [What this does not do](#what-this-does-not-do): the model's provider sees what the agent reads, prompt injection, the user's own processes. |
| A developer doesn't need to care about keys | The keys stay in the SDK's keystore or key file; the person pairs once. |

## Follow-up tickets

These tickets build the design. None is scheduled yet:

- **Declared tools in the schema and the lock**: `nc.tools` and its parts in the TypeScript SDK, the build's check in Rust, the lock's `tools` and RFC 0005's text on them, with a test per refusal of [What the build checks](#what-the-build-checks).
- **The server's skeleton**: the crate `ncdb-mcp`, `init`, `pair`, `token`, the host with its lock file, `stdio` that forwards or hosts, and `serve`, with tests that two `stdio` commands share one host and that the last client's exit ends a host `stdio` started.
- **The loopback transport and its checks**: the bearer token, the refused `Origin`, the `Host` check, and the refusal of any other address, each with a test that a request without them is refused.
- **Serving the declared tools**: `list_spaces`, finds, gets and actions with their schemas, `sync`, `waitForSync`, cursor handles, `--read-only` and `toolsRefused`.
- **The generic tools** behind `--generic-tools`.
- **Text edits** in actions and the generic rich-text tools, after RFC 0013's anchored edits are built.
- **Shipping**: the binaries, checksums and attestations in the release workflow, signing and notarization, and `ncdb-mcp` on crates.io.
- **The docs**: a page on declaring tools, and one that sets the server up for Claude Code and Codex and says what it does not do.

## How this RFC relates to the others

It builds a member on the Rust SDK of [RFC 0009](0009-sdk-api.md) and pairs it as an agent of [RFC 0010](0010-accounts.md); it adds declared tools to the schema and the lock of [RFC 0005](0005-schema-and-model-lock.md), which the follow-up ticket writes into RFC 0005's text; it reads the view of [RFC 0006](0006-local-view.md) and syncs through the relay of [RFC 0007](0007-relay.md) as any member does; it keeps the keys as [RFC 0003](0003-keys-and-cryptography.md) keeps them; and it exposes the anchored edits of [RFC 0013](0013-rich-text.md).

## Decisions

- **2026-11-24 · Declared tools live in `model.lock.json` under `tools`, and a lock that declares tools has `format` 2.** ([decision](../../factory/decisions/2026-11-24-NOC-504-declared-tools-live-in-the-lock-under-tools-with-lock-format-2.md))
- **2026-11-24 · The schema declares tools with `nc.tools`, `nc.tool.*`, `nc.param.*` and `nc.op.*`, and a lock with bad tools is refused with `toolsRefused`.** ([decision](../../factory/decisions/2026-11-24-NOC-504-the-schema-declares-tools-with-nc-tool-nc-param-and-nc-op.md))
- **2026-11-24 · A declared tool's name is its key in `nc.tools`, as the app writes it.** ([decision](../../factory/decisions/2026-11-24-NOC-504-a-declared-tool-s-name-is-its-key-as-the-app-writes-it.md))
- **2026-11-24 · The generic tools wait behind `--generic-tools`, and `list_spaces` is always listed.** ([decision](../../factory/decisions/2026-11-24-NOC-504-generic-tools-wait-behind-generic-tools-and-list-spaces-is-always-listed.md))

## History

- 2026-11-23 · First published (NOC-485)
