---
type: fix
tickets: [NOC-571]
introduced: cycle
components: [sdk-ts]
---
## Symptom
In a browser, `nc.open({ relay: () => Promise<string> })` never connected: `db.sync.current()` stayed `connecting`, and the relay function was never called. A fixed relay URL was not affected.

## When
In every browser, whenever `relay` is a function.

## Cause
The core asks for the relay URL as soon as the database opens, before the SDK listens to the core's events. The SDK dropped that first ask, and the core kept waiting for the URL.

## Fix
The SDK holds the core's asks, such as the relay URL, until it listens, and then answers them in order. The relay function is called at open and before each later connect.
