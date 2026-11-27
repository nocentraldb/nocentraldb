---
type: fix
tickets: [NOC-519]
introduced: 0.1.0
components: [sdk-rust, sdk-ts]
---
## Symptom

While a burst of writes went up to the relay, the writing device's memory grew with the changes still waiting to go up: a burst of 300,000 records, 500 to a write, peaked at 139 MB on the writer natively and took 70 MiB of the core's WebAssembly memory in a Chromium tab.

## When

When a device writes faster than it pushes, such as during an import, or when it reconnects after working offline with many changes the relay has not confirmed.

## Cause

Each push read and decoded every change of the space the relay had not confirmed yet, though one push carries at most 32 of them.

## Fix

The push reads the changes it sends a push at a time, at most 32 at once. The same burst now peaks at 55 MB on the writer natively and takes 44 MiB of the core's WebAssembly memory in a Chromium tab. The changes go up in the same pushes and the same order as before; nothing on disk or on the wire changes.
