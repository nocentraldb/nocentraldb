---

title: API
description: The main calls of the TypeScript SDK in 0.1, as RFC 0009 defines them.
---

[RFC 0009: The SDK API for apps](https://github.com/nocentraldb/nocentraldb/blob/main/docs/rfc/0009-sdk-api.md) defines the API, and both SDKs follow it. The calls below are the main ones of the TypeScript SDK in 0.1; the Rust SDK has the same calls in Rust's style, such as `db.sync().current()`. The SDK's complete public API is in the [API reference](/docs/api/reference/).

| Call                                      | What it does                                                                                    |
| ----------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `nc.open({ model, lock, name, relay })`   | Opens the database, checks the model against the lock, creates this member's key on first open. |
| `db.spaces.create()`                      | Creates a space on this device, offline if need be.                                             |
| `space.write(tx => …)`                    | One signed, encrypted change; resolves once it is in the local view.                            |
| `db.find(query)`                          | A page of a declared index, in index order, with a cursor.                                      |
| `db.subscribe(query, fn)`                 | Calls `fn` with the page now and after every change that may alter it.                          |
| `db.invitation()`                         | This member's invitation, which an admin adds to a space.                                       |
| `space.members.add(invitation, { role })` | Adds a member; returns a notice of what the change does not do.                                 |
| `db.sync.current()`                       | The connection and how many of this member's changes the relay has not confirmed.               |
| `db.durability.current()`                 | What this browser copy could lose, and whether a tab waits for another tab.                     |
