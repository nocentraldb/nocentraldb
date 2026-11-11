---

title: Copies and convergence
description: A full copy on every member, and the same state on all of them.
sidebar: { order: 3 }
---

Every member is designed to keep the complete encrypted log of each space it belongs to and to **fold** it, that is, apply each change in turn, into a local SQLite view. Reads are local; they never wait for the network.

Writes **converge**: the same set of changes yields the same state on every member, in any order of arrival. Each field type says how concurrent edits merge. A change that loses to one its author had not seen stays identifiable as such, the same way on every member.

In **island mode**, with no relay at all, members keep working. Network gone? Your writes are stored locally and sync when the relay is back.
