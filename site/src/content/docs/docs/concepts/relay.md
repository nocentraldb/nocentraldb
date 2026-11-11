---

title: The relay
description: A server that stores and forwards changes it cannot read.
sidebar: { order: 2 }
---

The relay checks signatures and membership, appends signed, encrypted changes to a space's log and hands them out from a cursor. It holds no keys, so it cannot read content.

**What it does see.** Which key is in which space, when and how much each key writes, the key generations and when they change, the rhythm of work, and who reads which space from where. It can also withhold changes or show members different subsets of a log.
