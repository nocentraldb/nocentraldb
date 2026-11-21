---

title: The relay
description: A server that stores and forwards changes it cannot read.
sidebar: { order: 2 }
---

The **relay** checks signatures and membership, appends signed, encrypted changes to a space's log and hands them out from a cursor. It holds no keys and folds nothing, so it cannot read content.

**What it does see.** Which key is in which space, when and how much each key writes, the key generations and when they change, the rhythm of work, and who reads which space from where. It can also withhold changes or show members different subsets of a log.

**Where it runs in 0.1.** Locally, as `ncdb-relay` on SQLite, for development and small setups; or serverless on AWS, with DynamoDB and S3, as [RFC 0007](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0007-relay.md) designs it. Both run the same relay core, so both make the same checks. Postgres comes later.
