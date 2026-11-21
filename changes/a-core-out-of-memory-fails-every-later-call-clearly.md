---
type: fix
tickets: [NOC-426]
introduced: 0.1.0
components: [sdk-ts]
---
## Symptom
In the browser, after a write ran out of the core's WebAssembly memory, every later call on that database waited forever, and the page could show an uncaught `RuntimeError: unreachable`.

## When
A write of one value from about 200 MiB in Chromium, or any other failure that stops the core in its Web Worker.

## Cause
WebAssembly traps when it runs out of memory, and a trap in a step the core ran after the call returned reached nobody: the call's promise never settled, so the worker answered nothing more.

## Fix
The worker catches every trap of its core. The call that failed gets an `nc.Error` with the code `storageFailed`; every later call gets `storageFailed` with a message that says to close the database and open it again, which starts the core afresh with everything written before. No error reaches the page uncaught. [Browser apps and durability](../docs/browser-durability.md#a-write-too-large-for-the-tabs-memory) names the limit.
