# NoCentralDB FAQ

This page answers the questions that [RFC 0001](rfc/0001-nocentraldb.md) and the [use cases](use-cases.md) raise most often. NoCentralDB is not released yet. Where the design does not have an answer yet, this page says so and names the open question in the RFC that will decide it. If your question is missing, ask it in the RFC discussions on GitHub.

**Status and use**

- [Can I use NoCentralDB today?](#can-i-use-nocentraldb-today)
- [Under which licence is it published?](#under-which-licence-is-it-published)
- [Which platforms and languages does it support?](#which-platforms-and-languages-does-it-support)

**Choosing it**

- [When should I use a central database instead?](#when-should-i-use-a-central-database-instead)
- [How does it differ from other sync engines?](#how-does-it-differ-from-other-sync-engines)
- [Why not use a CRDT library such as Yjs or Automerge directly?](#why-not-use-a-crdt-library-such-as-yjs-or-automerge-directly)
- [Can I move an existing app to NoCentralDB?](#can-i-move-an-existing-app-to-nocentraldb)

**Data and queries**

- [Is it a SQL database?](#is-it-a-sql-database)
- [What happens when two members change the same data at the same time?](#what-happens-when-two-members-change-the-same-data-at-the-same-time)
- [Can I enforce unique values, such as usernames?](#can-i-enforce-unique-values-such-as-usernames)
- [Does every device have to store all the data?](#does-every-device-have-to-store-all-the-data)
- [How do I change the schema?](#how-do-i-change-the-schema)
- [How fast is it?](#how-fast-is-it)

**Keys, rights and the relay**

- [What can the relay see?](#what-can-the-relay-see)
- [Who runs the relay?](#who-runs-the-relay)
- [What happens if I lose my device?](#what-happens-if-i-lose-my-device)
- [How fine-grained are the rights?](#how-fine-grained-are-the-rights)
- [How do agents become members, and how do I remove them?](#how-do-agents-become-members-and-how-do-i-remove-them)
- [My feature needs a server that can read the data. What then?](#my-feature-needs-a-server-that-can-read-the-data-what-then)

**Erasure and compliance**

- [Does destroying a key satisfy a GDPR erasure request?](#does-destroying-a-key-satisfy-a-gdpr-erasure-request)
- [What happens to the audit trail after an erasure?](#what-happens-to-the-audit-trail-after-an-erasure)

**Taking part**

- [How can I give feedback or contribute?](#how-can-i-give-feedback-or-contribute)

---

## Status and use

### Can I use NoCentralDB today?

Not yet. We have started building the core, and the RFC describes the design we are building toward. The code in the RFC and the use cases is an API we propose, and it may change. Watch the repository's releases to hear when the first versions come out. Early versions appear as alpha and beta releases before a final release.

### Under which licence is it published?

The code is dual-licensed under the MIT licence and the Apache licence 2.0, at your choice. Contributions are licensed the same way. The licence lets anyone run a relay as a service, and we welcome companies that offer hosting. We only ask them to take an active part in developing NoCentralDB.

### Which platforms and languages does it support?

The core is written in Rust. It runs natively and, compiled to WebAssembly, in the browser. The design has two SDKs, one for Rust and one for TypeScript. You describe the schema once in TypeScript, and both SDKs read the same generated `model.lock.json`. The RFC does not yet specify native mobile SDKs.

## Choosing it

### When should I use a central database instead?

Use a central database when your app needs queries on the server, analytics across all users' data, or one global truth enforced at write time, such as unique names or transactions across all users. NoCentralDB does not offer these, on purpose. It is for apps where people and agents work on shared data, where the server should not be able to read that data, and where every member can hold a full copy of the spaces it works in.

### How does it differ from other sync engines?

Many sync engines keep a trusted server. That server can read the data, enforces fine-grained rights, and often answers queries. NoCentralDB replaces it with a relay that holds no keys and cannot read content. The relay only stores and forwards signed, encrypted changes. Every member decrypts and queries its own copy. The cost is coarser rights, because rights apply per space. The [use cases](use-cases.md) show where that trade-off pays off.

### Why not use a CRDT library such as Yjs or Automerge directly?

A CRDT library merges concurrent edits, and NoCentralDB uses Yjs for long text. A database for teams needs more around the merge: spaces with membership and keys, encryption, a relay, declared indexes and views, computed fields, ordered pages, work orders such as compaction and erasure, and a schema shared by two SDKs. NoCentralDB provides these parts as one system, so your app does not have to build them around a library.

### Can I move an existing app to NoCentralDB?

The design does not include a migration tool yet. Moving an app means modelling its data as collections in a NoCentralDB schema and deciding how to split it into spaces. Data that needs server-side queries or global uniqueness should stay in a central database. Both can run side by side.

## Data and queries

### Is it a SQL database?

Not from the app's point of view. Each member folds its encrypted log into a local SQLite database, but your app queries through the SDK. You declare indexes, views and computed fields in the schema, and queries run only through those declarations. Every read is therefore predictable. A page of results is one range lookup.

### What happens when two members change the same data at the same time?

Both changes are kept, and every copy converges to the same result, whatever order the changes arrive in. Each field's type declares how it merges. For basic values the last writer wins, sets and links combine, and rich text merges the edits of both members. A field type you add yourself brings its own merge rules. Computed fields follow from the merged values, so they converge too.

### Can I enforce unique values, such as usernames?

No. Enforcing a unique value needs one place that sees every write before it is accepted, and NoCentralDB has no such place. Members write locally and converge later. If your app needs globally unique names, keep that part in a central database.

### Does every device have to store all the data?

Every member stores the full, encrypted log of each space it belongs to, not of the whole database. A device that belongs to few or small spaces stores little. Compaction keeps the log short: it moves old history, encrypted and signed, into an archive, so a member does not replay a long history. The RFC assumes that a phone can store a team's complete working set. If one space grows too large for your devices, split it into several spaces.

A browser keeps its copy only as long as the browser allows. It may delete a web app's storage when storage runs low, and Safari deletes a site's script-writable storage after seven days of use without the user interacting with the site ([RFC 0006, "The browser"](rfc/0006-local-view.md#the-browser)). The member key goes with it, so the browser has to be set up again as a new device, and the changes it wrote that the relay had not confirmed are lost ([RFC 0001, "What it is not"](rfc/0001-nocentraldb.md#what-it-is-not)). The SDK asks the browser for persistent storage and tells your app whether it was granted and how many unconfirmed changes the copy holds ([RFC 0006, "What the SDK reports"](rfc/0006-local-view.md#what-the-sdk-reports)). [Browser apps and durability](browser-durability.md) says per browser what is measured, what is documented and what is unknown, and [How long a browser keeps a web app's copy](browser-storage.md) has the measurements.

### How do I change the schema?

You change the TypeScript model and run `ncdb model build`, which writes a new version of `model.lock.json`. Schema changes are additive, and old clients keep data they do not understand yet, so members with different app versions can work on the same space. The RFC does not yet describe how to remove or rename a field.

### How fast is it?

We have no published measurements yet. By design, reads never leave the device, and a page of results from a declared view is one range lookup in a local SQLite database, with no network round-trip. Writes are local first and reach other members as fast as the relay forwards them. We will publish measurements when the core runs.

## Keys, rights and the relay

### What can the relay see?

The relay cannot read content, because it holds no keys. It checks signatures and membership, appends the encrypted changes to each space's log, and hands them out, so a copy of the relay's database contains no readable content. It does see metadata: which key is a member of which space, when and how much each key writes, the key generations and when they change, the rhythm of work, which member reads which space from which network address, and the ids of key envelopes and erasures. It can also withhold changes or show different members different subsets of a log. [RFC 0002, "What the relay can see, and what it cannot"](rfc/0002-change-and-log-format.md#what-the-relay-can-see-and-what-it-cannot) lists what it learns from the log, and [RFC 0007, "What the relay sees"](rfc/0007-relay.md#what-the-relay-sees) adds what it learns from its protocol, such as when each member is online and which client it runs, and what the server it runs on sees.

The relay applies the membership rules itself, so it reads the control changes that carry them: who adds and removes members, key envelopes, key rotations and erasures. From them it learns each member's role and when it changes, which control changes each one builds on and so how up to date each admin was, the cutoff of each revocation (how many of a removed member's changes the remover had received), the account key behind each device and agent, which links one person's devices and agents across every space on that relay, which member is a sub-agent of which, which accounts sign off erasures, and when concurrent key rotations happened. It still learns no content, no space name and no model id. [RFC 0008, "How the relay applies the rules"](rfc/0008-membership.md#how-the-relay-applies-the-rules) lists it.

### Who runs the relay?

That is open, and the RFC asks for your view in question 6, "Hosting the relay". The options are running it yourself on Postgres, running it serverless in your own cloud account, or using a hosted relay that can only ever see ciphertext. We design the relay to run on SQLite, Postgres, or DynamoDB with S3. Because the relay holds no keys, a hosted relay can read no more than one you run yourself. Whether we will offer hosting ourselves is open. We welcome companies that offer it, and we ask them to take an active part in developing NoCentralDB.

### What happens if I lose my device?

Your data is not lost as long as another member of the space, or another of your own devices, still holds a copy. Every member keeps the full log of its spaces, and the relay keeps the encrypted log.

The lost device gives away what it holds. Its local view keeps every space its members belong to in plaintext, so that reads are fast, and whoever reads the unlocked device or its unencrypted disk reads all of it ([RFC 0006, "What the view stores in plaintext"](rfc/0006-local-view.md#what-the-view-stores-in-plaintext)). The private keys are not in that database but in the platform's keystore, and in the browser a WebCrypto key that cannot be exported wraps them. v1 does not encrypt the database itself: it relies on the device's disk encryption, such as FileVault, BitLocker, LUKS, or the encryption of iOS and Android ([RFC 0006, "What the view stores in plaintext"](rfc/0006-local-view.md#what-the-view-stores-in-plaintext)). Removing the lost device's member from your spaces stops it from receiving or writing new changes, but it does not delete the copy the device already holds ([RFC 0003, "Removing a member and rotating the space key"](rfc/0003-keys-and-cryptography.md#removing-a-member-and-rotating-the-space-key)).

If you lose every device, the recovery code you keep makes a new account key under the same account on a new device, with the same fingerprint, and that account key certifies the new device's member key ([RFC 0003, "The account key"](rfc/0003-keys-and-cryptography.md#the-account-key); [RFC 0010, "The recovery code"](rfc/0010-accounts.md#the-recovery-code)). It restores the account, not the spaces: another member of each space adds the new device again.

### How fine-grained are the rights?

Rights apply per space. Every object belongs to exactly one space, and every member of a space can read the whole space. For finer rights, you use many small spaces. A member is a person's device, an agent or a sub-agent. There are no groups for now: to give several members the same spaces, an app adds each of them to each space, and `members.add` takes several invitations in one call ([RFC 0001, "Groups leave the design"](rfc/0001-nocentraldb.md#groups-leave-the-design)). Whether that is enough for v1, or whether field-level write scopes are needed, is question 1 in the RFC, "Rights per space".

### How do agents become members, and how do I remove them?

An agent joins a space like any other member, with its own key and its own role. It signs every write with that key, so everyone can see what the agent changed. To remove an agent, you remove it from the space, and the space's key rotates, so the agent receives no new changes. It keeps what it already received. An agent that runs as a user on that user's own device is not a separate member, and it can read whatever the user can read.

### My feature needs a server that can read the data. What then?

Some features need a server that reads content, for example rendering a page on the server or sending an email digest. We plan to make such a server a regular member of the spaces it needs, with its own key and visible in the member list, instead of offering a hidden "server-readable" mode. Whether that would pass your review is question 2 in the RFC, "Server-readable data". [Use case 7](use-cases.md#7-subscribe-instead-of-poll) shows the same model for a cloud function.

## Erasure and compliance

### Does destroying a key satisfy a GDPR erasure request?

Your data protection officer decides that for your case; this is a design, not legal advice. In the design, the SDK encrypts each person's marked personal fields with that person's own key, and an erasure destroys that key. Without the key, those fields are unreadable in every archive and old snapshot and on every device that has deleted the key. The erasure does not reach devices that never come back online, a modified client that ignores the erasure and keeps the key, a member of the space who obtains a relay backup taken before the erasure, copies made outside the database such as exports and screenshots, or personal data in unmarked free text ([RFC 0003, "Shredding a person's key"](rfc/0003-keys-and-cryptography.md#shredding-a-persons-key)). Keeping names out of free text is your app's job: it stores a reference to the person instead of the name and resolves the reference when it displays the text, so that after an erasure the text shows a placeholder; a name typed or pasted as plain text stays ([RFC 0005, "Personal data in free text"](rfc/0005-schema-and-model-lock.md#personal-data-in-free-text)). Removing a member is not erasure: it blocks the member's access from then on, while the data the member wrote and the personal fields about them stay ([RFC 0005, "About whom, not by whom"](rfc/0005-schema-and-model-lock.md#about-whom-not-by-whom)). The key is also destroyed only once the erasure is stable, when every admin device of the space has written a change after seeing it. Every view shows the person's fields as erased at once, but an admin device that stays offline keeps the key alive on the members until it comes back or is demoted; after 7 days the SDK of every admin raises an alarm that names it ([RFC 0008, "Effects that wait until a change is stable"](rfc/0008-membership.md#effects-that-wait-until-a-change-is-stable)). What evidence your officer would need is question 3 in the RFC, "Erasure evidence".

### What happens to the audit trail after an erasure?

It stays intact. An erasure is a work order: one member performs it, and a second account signs it off. Destroying the person's key makes their fields unreadable but leaves the signatures valid. The audit trail therefore still shows who changed what and when, and who signed off the erasure. If no eligible member comes online before a deadline, the RFC has not decided yet how to escalate. That is question 5, "Nobody online".

## Taking part

### How can I give feedback or contribute?

Post in the RFC discussions on GitHub. Describe your app, your situation and where the design would break for you; reports of problems help us more than feature requests. We read every reply, answer within a week, and publish a summary of the changes in the next draft. To propose a change to the RFC text, open a pull request against it. For bug reports, offers to take on a roadmap milestone, and how pull requests work here, see [CONTRIBUTING.md](../CONTRIBUTING.md).
