---
type: fix
tickets: [NOC-425]
introduced: 0.1.0
components: [sdk-ts]
---
## Symptom
In the browser, `space.write` with a value of about 125 KiB or more failed with `RangeError: Maximum call stack size exceeded` instead of reaching the core, and after `db.close()` a page could show the error "BroadcastChannel is closed".

## When
Any write with a large string or bytes value, and closing a database whose tab answered another tab's call or got an event from its worker while it closed.

## Cause
The SDK's encoder for the worker bridge copied each byte of a value as a separate function argument, which exceeds the stack for large values. The tab layer posted answers and events on its BroadcastChannel after it had closed it.

## Fix
The encoder copies bytes into a growing buffer, so a value of any size the core holds goes through; a value too large for the core's memory fails with an `nc.Error` with a code (`storageFailed` in Chromium from about 200 MiB). A closed tab posts nothing more.
