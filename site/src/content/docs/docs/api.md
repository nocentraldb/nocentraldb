---

title: API
description: The SDK API for apps, as RFC 0009 defines it.
---

The SDK API is designed in [RFC 0009: The SDK API for apps](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0009-sdk-api.md). This page will hold the reference once the SDK exists. Every call below is designed, not built.

| Call                                      | What it does                                                                                    |
| ----------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `nc.open({ model, lock, relay })`         | Opens the database, checks the model against the lock, creates this member's key on first open. |
| `db.spaces.create()`                      | Creates a space on this device, offline if need be.                                             |
| `space.write(tx => …)`                    | One signed, encrypted change; resolves once it is in the local view.                            |
| `db.find(query)`                          | A page of a declared index, in index order, with a cursor.                                      |
| `db.subscribe(query, fn)`                 | Calls `fn` with the page now and after every change that may alter it.                          |
| `space.members.add(invitation, { role })` | Adds a member; returns a notice of what the change does not do.                                 |
| `db.sync.current()`                       | The connection and how many changes the relay has not confirmed.                                |
