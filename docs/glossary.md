# Glossary

This page sets the words that NoCentralDB's texts use: the website, the SDK's, the documentation and the RFCs. When a text needs one of these concepts, it uses the term in the first column, as the definition uses it. It does not use any of the words in "Avoid".

How to read it:

- **Term** is the word we write. Code names, where they differ, follow in backticks.
- **Avoid** lists the words a reader might expect instead, and that we do not use for this concept. Some of them are right for another concept; then the row says which.
- **0.1** says when a concept is not in release 0.1 yet. A text never describes such a concept as something that works today.

## The product

| Term | Meaning | Avoid |
| --- | --- | --- |
| NoCentralDB | The product, written as one word with three capitals. The packages are `@nocentraldb/sdk` and `nocentraldb`, the command-line tool is `ncdb`. | NoCentralDb, Nocentraldb, NCDB, "No Central DB" |
| local-first database | What NoCentralDB is: every member works on its own full copy, and can read even if the network isn't available. | offline-first database, sync engine, CRDT database (all three are close, and none says what we mean) |
| SDK | An app uses the following libraries (SDKs): the TypeScript SDK (`@nocentraldb/sdk`) or the Rust SDK (`nocentraldb`). In 0.1 the TypeScript SDK runs in the browser only. | client library, driver, client (a "client" is any program that talks to the relay) |
| core | The Rust code below both SDKs (`ncdb-core`). It runs natively and as WebAssembly in the browser. The same core everywhere: behaviour never forks by platform. | engine (except in "the engine's field types"), runtime, backend |
| `ncdb` | The command-line tool. In 0.1 it builds and checks the lock (`ncdb model build`, `ncdb model check`). | the CLI, when the name fits ("`ncdb model build`", not "the CLI command") |
| pre-release | An alpha or a beta. Alphas come after every product change on `main`, betas and releases by tag ([release management](release-management.md)). | preview, early access, launch |

## Spaces and members

| Term | Meaning | Avoid |
| --- | --- | --- |
| space | The unit of sharing, membership and keys. Every object belongs to exactly one space. Fine-grained rights come from many small spaces. | workspace, room, project, tenant, channel (a "project" can be what an app keeps in a space); group, which left the design ([RFC 0008, "Groups"](rfc/0008-membership.md#groups)) |
| member | Whoever holds a copy of a space and writes to it under its own key: a person's device, an agent or a sub-agent. A member is never a person. | user (a person, who may have several devices), participant, peer, node |
| role | What a member may do in a space: `reader`, `writer` or `admin`. | permission, right, access level |
| device | A person's phone, laptop or browser, with its own database and its own key. One database is one member. | client, installation, instance |
| agent | A program that works on data on its own and is a member, with its own key and its own copy, visible in the member list. | bot, assistant, AI (as a noun), service account |
| sub-agent | An agent that another agent starts for one task. It gets an ad-hoc space with only what the task needs, never its parent's keys. | subagent, child agent, worker (a "worker" is the browser's Web Worker) |
| ad-hoc space | A space an agent creates for a sub-agent, holding copies of only the objects the task needs. | temporary space, scratch space, sandbox |
| account | One person's identity across their devices and agents, with an account key and a fingerprint. Devices of one account pair with each other; a second account signs off an erasure. | user, profile, identity, login |
| account space | The space that records an account's devices and agents, its account key statements and the spaces each device belongs to. Its members are the account's devices, each an admin. Its records are sealed content the relay cannot read. | account database, profile |
| certificate | The account key's signature that a member belongs to an account, as a device or as an agent. An invitation carries it. A sub-agent's certificate is signed by its parent agent instead. | credential, attestation |
| invitation | The text a joining member gives an admin: its public keys and its account's fingerprint, never a secret (`ncdb-invite-1:…`). The joining member creates it; an admin adds it. | invite (as a noun), invite link, join code, token |
| fingerprint | A short value that identifies an account. A person compares it, as messaging apps compare safety numbers, before an admin adds the account's first member. | safety number, key ID, hash |
| pairing | Adding a second device to one's own account. | linking (as a noun), syncing devices, device login |
| pairing answer | The text a device that holds the account key gives the device it pairs (`ncdb-pair-1:…`). It carries the account key sealed to that device, so only that device opens it. | pairing code, pairing token |
| recovery code | Twelve words that let a person open a new device of their account when every device is lost. | recovery phrase, seed phrase, backup code, mnemonic |
| notice | The plain-language text, with a code, that every call changing who can read returns, saying what the change does not do (`{ code, text }`). | warning, disclaimer, message |

## Data and the schema

| Term | Meaning | Avoid |
| --- | --- | --- |
| model | The schema of an app's data, written once in TypeScript with `nc.model`. | data model (fine in prose where "model" alone is ambiguous), entity model |
| schema | The description of collections, fields and indexes, in general. "The model" is an app's schema in code. | DDL, table definition |
| lock | `model.lock.json`, which `ncdb model build` writes from the model and which the core and both SDKs read. Write "the lock" or `model.lock.json`. | lockfile (that is the package manager's), schema file, compiled schema |
| model version | The number the lock carries. `ncdb model build` raises it when the model changes, except in its descriptions. | schema version |
| deprecated | A collection or field marked `.deprecated(…)`: it stays in the model, and the app stops writing it. A model removes nothing; it deprecates. | obsolete, retired (removing is something else, and the lock refuses it) |
| collection | A kind of object in the model, such as `tasks`. | table (the local view has tables; the model has collections), type, entity |
| object | One item of a collection, with an id, in one space. | record, row (a row is in the local view's tables), document (a document is rich text), entity, item |
| field | One named value of an object, with a field type. | column, property, attribute |
| field type | What a field holds and how concurrent changes to it merge: basic values, sets, links, references and personal fields in 0.1. Plugin field types ([RFC 0011](rfc/0011-plugin-field-types.md)) let a developer add their own. | data type, kind |
| personal field | A field marked `.personal(…)`, encrypted with the person key of the person it is about, so that erasing that person makes it unreadable. | PII field, private field, sensitive field |
| link | A field that points to another object in the same space (`nc.link`, `nc.links`). | reference (a reference is the field that may point into another space), relation, foreign key, pointer |
| reference | A field that points to one object, possibly in another space (`nc.ref`); its Rust value is `Reference { space, id }`. It gives no access to the target's space. | link (a link stays in its space), pointer |
| rich text, stream | Field types that are reserved in the schema and come after 0.1. | **0.1:** never described as working |
| index | An order the model declares over a collection's fields. Every query runs through one. | view (a view is the local copy's computed result) |
| query | A read through one declared index, with its leading fields fixed, answering one page. | filter, search, lookup |
| page | The objects a query answers, in index order, with a cursor for the next page. | result set, batch, chunk |
| subscription | A listener that `subscribe` calls at once with the current page and again after every change that may alter it. | live query, observer, watch |

## Changes, the log and convergence

| Term | Meaning | Avoid |
| --- | --- | --- |
| change | One write: signed, encrypted, in one space. A `write` with several creates and updates is one change; other members receive all of it or nothing. | mutation, operation, op, event, transaction (a `tx` is how an app records a change), commit, update (an update is one kind of edit within a change) |
| log | The encrypted, append-only sequence of a space's changes. | history (fine in prose), journal, event log, stream (a stream is a field type) |
| copy | The log of every space a member belongs to, and the view it folds into, on that member's machine. "A full copy" in the hero's sense. | replica, cache, mirror, clone |
| fold | To apply each change of the log in turn to the view. Used in the docs; the website says "keeps its copy up to date". | replay, materialize, project |
| view | The SQLite database a member folds its log into. Apps read only from it, never from the log or the network. Also "local view". | cache, read model, projection, materialized view |
| converge | The same set of changes yields the same state on every member, in any order of arrival. | sync up, reconcile, agree, eventually consistent (correct, but say what it means instead) |
| conflict | Two changes to one field that neither author had seen. Each field type says how they merge; a change that loses stays identifiable as such. | collision (except in the footer line "No collisions"), clash, race |
| island mode | Working with no relay at all: members keep working, and every copy converges when they reconnect. | offline mode (what one device does is just "offline"), airplane mode |
| unconfirmed change | A change of this member that the relay has not confirmed yet, so it exists only on this device. | pending change, unsynced change, local change, outbox (the outbox is RFC 0012's internal store) |

## The relay

| Term | Meaning | Avoid |
| --- | --- | --- |
| relay | The server that checks signatures and membership, appends encrypted changes to a space's log and hands them out from a cursor. It holds no keys and cannot read content; it does see metadata. In 0.1 it runs locally on SQLite (`ncdb-relay`) or serverless on AWS (DynamoDB and S3). | sync server, backend, server (but see the exception below), hub, cloud, broker |
| metadata | What the relay sees: which key is in which space, when and how much each key writes, the key generations and when they change, the rhythm of work, and who reads which space from where. | traffic data, telemetry |
| sync | The background exchange of changes between a member and the relay. `db.sync` reports the connection and the unconfirmed changes. | replication, push and pull, upload |

**Exception: "server" on the landing page.** The landing page's hero and its scenario explain the mechanism in plain words, and there the relay is "a server in the cloud", drawn as "Cloud server" (Carsten, 2026-11-12, "The landing page leads with speed, in two colours"; `site/scripts/landing.test.mjs` holds it). From the code section on, the page says "relay", and so does every other text.

## Keys and erasure

| Term | Meaning | Avoid |
| --- | --- | --- |
| key | Written as the kind of key when it matters: member key, space key, account key, person key. The SDK manages every key; an app never handles one. | secret (a secret is what a key protects), password, credential |
| member key | The key a member signs its changes with, created on first open. In the browser it is sealed in IndexedDB under a non-extractable WebCrypto key. | device key, identity key |
| keystore, key file | Where the Rust SDK keeps a member's keys: the platform's keystore, or a key file the app names with `key_file`. The browser keeps them in IndexedDB. | keychain (except macOS's own), secret store |
| space key | The key a space's content is encrypted with. Removing a member rotates it. | room key |
| rotation, rotate | Replacing a space key with a new generation, chained to the earlier ones, so that a member that left or was removed cannot read what is written after. | rekey, key rollover |
| person key | One key per person per space, which encrypts that person's personal fields. | user key, PII key |
| erasure, erase | Making a person's personal fields unreadable by destroying their person keys, in every space of the database where the member may write (`Db::erase`, notice `Erased`). In 0.1, the Rust SDK erases; the browser SDK reads the erasure audit. | delete (deleting an object is something else), forget, wipe, purge, shredding (as the name of the whole act) |
| shred | Destroying one person key in one space, the step of an erasure in one space (`Erased.shreds`). Used in the RFCs and the API, not as the reader's name for the act. | burn, revoke (revoking is what removing a member does) |
| sign-off | The approval of a second account that an erasure needs, carried as text between the apps (`ncdb-erase-1:…` request, `sign_off`). With only one account in a space, the erasure is provisional for 7 days and can be taken back. | approval, co-sign, countersignature |
| erasure request | The text (`ncdb-erase-1:…`) a device that performed an erasure gives a second account to sign off. | erase request, deletion request |
| erasure audit | `space.erasures.list()`: for each erasure in a space, whether it is final, provisional or taken back, who performed it and who signed it off, and which members confirmed that they destroyed the key. Every member may read it. | erasure log, deletion record |
| work order | A clean-up job that one member claims and another validates. | **0.1:** not in 0.1; never described as working |
| compaction, archive | Moving history into a signed, encrypted archive. | **0.1:** not in 0.1; never described as working |

## The browser

| Term | Meaning | Avoid |
| --- | --- | --- |
| browser | Where the TypeScript SDK runs in 0.1. Name engines as Chromium, Firefox and Safari; the tests run in Playwright's Chromium, Firefox and WebKit, and WebKit is not Safari. | web (as a noun), Chrome (unless the branded browser is meant) |
| database | What `nc.open` opens: one member's copy and keys, under a name, in this origin's private file system (OPFS). | store, DB (in prose), storage (the browser's storage is where the database lives) |
| tab | A browser tab of the app. In 0.1 several tabs share one database: one tab owns it and the others reach it through that tab. In Safari, a tab whose owner is frozen waits (`waitingForTab`). | window (a private window is something else), page (a page is a query's answer) |
| private window | A browser window that keeps no data. In Firefox and Safari it has no OPFS, and `nc.open` refuses with `storageUnavailable`; in Chromium the database is deleted when the window closes. | incognito (name it only for Chrome), private mode |
| durability | What a browser copy could lose: `db.durability` reports `persistent` or `best-effort` storage, the unconfirmed changes and `waitingForTab`. | persistence (except the browser's "persistent storage"), safety |

## Words we do not use for NoCentralDB

- **"Serverless"** for NoCentralDB itself: there is a relay. The AWS relay runs on serverless services, and only that may be called serverless.
- **"End-to-end encrypted"** without saying who the ends are: write "the relay cannot read content" and name what it sees.
- **"Zero-knowledge"**: the relay sees metadata.
- **"Real-time"**: a subscription answers after every change; the network decides when a change arrives.
- **"Blockchain", "decentralized ledger"**: no consensus, no chain.
- **"Secure"** on its own: say which guarantee, and its limits.
