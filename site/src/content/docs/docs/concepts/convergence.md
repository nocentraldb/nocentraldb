---
title: Copies and convergence
description: A full copy on every member, and the same state on all of them.
sidebar: { order: 3 }
---

Every member keeps the complete encrypted log of each space it belongs to and folds it into a local SQLite view. Reads never wait for the network.

The same set of changes yields the same state on every member, in any order of arrival. Each field type says how concurrent edits merge. A change that loses to one its author had not seen stays identifiable as such, the same way on every member.

With no relay, members keep working; when they reconnect, every copy converges.
