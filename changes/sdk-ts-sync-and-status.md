---
type: feature
tickets: [NOC-176]
components: [sdk-ts]
---
## Problem

An app in the browser could open its database with a relay, but could not see whether it was connected, pause syncing, learn when the relay held a write, or see what a space held that a read does not show: parked or rejected changes, gaps, refusals, or a provisional erasure it must show.

## Use

Read the relay connection as a report, follow it, and pause or resume syncing. Wait for the relay to hold a write by its change id:

```ts
const db = await nc.open({ model, lock, relay: "wss://relay.example.com" });
db.sync.current(); // { connection: "connecting", relay: "wss://…", unconfirmed: 0, refused: 0, epochChanged: false }
const stop = db.sync.subscribe((status) => showConnection(status.connection));

const { change } = await project.write((tx) => { /* … */ });
await db.sync.confirmed(change); // may never resolve, if the relay refuses it or is never reached

await db.sync.pause();
await db.sync.resume();
```

Read a space's status from its handle, and follow it:

```ts
const project = await db.spaces.get(id);
project.status.current(); // { role: "admin", initialSyncComplete: true, behind: 0, parked: {}, … }
project.status.subscribe((status) => showProvisionalErasures(status.provisionalErasures));
```
