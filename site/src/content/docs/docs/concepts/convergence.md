---

title: Copies and convergence
description: A full copy on every member, and the same state on all of them.
sidebar: { order: 3 }
---

Every member keeps the complete encrypted log of each space it belongs to and **folds** it, that is, applies each change in turn, into a local SQLite view. Reads are local; they never wait for the network.

Writes **converge**: the same set of changes yields the same state on every member, in any order of arrival. Each field type says how concurrent edits merge. A change that loses to one its author had not seen stays identifiable as such, the same way on every member.

In **island mode**, with no relay at all, members keep working. When the network is gone, your writes are stored locally, and they sync when the relay is back.

In the browser, the copy lives in the origin private file system (OPFS), and the browser may delete it when storage runs low. `db.durability` reports what is at risk: [Browser apps and durability](https://github.com/nocentraldb/nocentraldb/blob/main/docs/browser-durability.md) says what each browser keeps.
