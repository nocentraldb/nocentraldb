---

title: How NoCentralDB works
description: The database on every member, spaces and members, a change through the relay, the view, the model and the keys, on one page.
---

NoCentralDB is a local-first database. Every member works on its own full copy, and a relay in the middle passes encrypted changes on without being able to read them. This page walks through the parts once, in the order a change meets them. Each section links to its concept page or its RFC for depth.

## A database on every member

A member is a person's device, an agent or a sub-agent. On every member, your app runs on an SDK, the TypeScript SDK or the Rust SDK, over the same Rust core. The core keeps the encrypted log of every space the member belongs to and folds it, change by change, into a local SQLite view. Your app reads only from that view, through the indexes your model declares, so a read never waits for the network.

![One member, from top to bottom: your app or agent; the SDK, TypeScript or Rust, which manages the keys; and the core, ncdb-core, which folds the encrypted log of every space the member belongs to into a local SQLite view. Beside the core sit the field types, as plugins, and model.lock.json, which the core and the SDKs read.](/diagrams/0001-one-member.svg)

More in [Copies and convergence](/docs/concepts/convergence/) and [RFC 0006, the local view](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0006-local-view.md).

## Spaces and members

A space is the unit of sharing, membership and keys. Every object belongs to exactly one space, and fine-grained rights come from many small spaces. A member has a role in each of its spaces: `reader`, `writer` or `admin`. A space's changes reach only its members; a member of another space receives nothing from it and holds no key to read it.

![Two spaces, Project A and HR, with their logs on the relay. Bob's phone writes a change in Project A. Agent X, a cloud function and Alice's laptop, members of Project A, decrypt it and fold it. Carol, a member of HR only, receives nothing from Project A and holds no key to read it.](/diagrams/0001-two-spaces.svg)

More in [Spaces and members](/docs/concepts/spaces/) and [RFC 0008, membership](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0008-membership.md).

## A change, signed and encrypted, through the relay

A write is one change in one space. The member encrypts it with the space's content key, signs it with its member key, and folds it into its own view at once: the write resolves before the relay has it. The relay checks the signature and that the author may write, appends the change to the space's log and hands it out from a cursor. Every other member checks the signature and that the author was a member, decrypts the change and folds it. A member that is offline gets the change when it reconnects.

The relay holds no keys and cannot read content. It does see metadata: which key is in which space, when and how much each key writes, and who reads which space from where.

![One change from Bob's phone to Alice's laptop through the relay, in eight steps. The phone encrypts the change with Project A's content key, signs it with its member key, and folds it into its own view. The relay checks the signature and that the author may write, and appends the change to Project A's log. The laptop checks the signature and that the author was a member when it wrote the change, decrypts it and folds it.](/diagrams/0001-one-change.svg)

More in [The relay](/docs/concepts/relay/), [RFC 0002, the change and the log](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0002-change-and-log-format.md) and [RFC 0007, the relay](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0007-relay.md).

## The view converges

Each field type says how concurrent changes merge, so the same set of changes gives the same state on every member, in any order of arrival. A change that loses to one its author had not seen stays identifiable as such, the same way on every member. With no relay at all, in island mode, members keep working, and every copy converges when they reconnect.

More in [Copies and convergence](/docs/concepts/convergence/) and [RFC 0004, merge rules](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0004-merge-rules-basic-types.md).

## The model and the lock

You write the model once, in TypeScript: its collections, their fields with a field type each, and the indexes every query runs through. `ncdb model build` turns it into `model.lock.json`, which the core and both SDKs read. Model changes are additive: the build refuses a change that clients on the previous lock could not keep, so a member that stays offline for weeks keeps working.

More in [RFC 0005, the schema and model.lock.json](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0005-schema-and-model-lock.md).

## Keys, handled by the SDK

The SDK creates, stores and rotates every key: the member key that signs a member's changes, the space key, whose generations encrypt the content, the account key that certifies a person's members, and a person key per person per space that encrypts that person's personal fields. Your code never touches a key; the one secret it passes on is the account's recovery code, which the person keeps. Removing a member rotates the space key, so the member cannot read new changes, but it keeps the copy it already holds. Erasing a person destroys that person's keys.

More in [Spaces and members](/docs/concepts/spaces/), [Erasing a person](/docs/concepts/shredding/) and [RFC 0003, keys and cryptography](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0003-keys-and-cryptography.md).
