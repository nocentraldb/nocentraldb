---
type: fix
tickets: [NOC-518]
introduced: 0.1.0
components: [core, sdk-rust, sdk-ts]
---
## Symptom

An open space kept about 1.4 KB of memory for each change in its log, whatever the change's size. Written one record to a change, 81,000 records held 120 MB of the Rust SDK's heap and 131 MiB of a Chromium tab's core memory, and 300,000 held 459 MB and 448 MiB: more than a phone or a browser tab can give one space.

## When

On every device with an open database, natively and in the browser, in proportion to the number of changes in its spaces' logs. It mattered most for spaces of many small changes.

## Cause

The core's in-memory log kept for each change a whole header, a one-entry set of its dependencies, and its author sequence and clock in separate maps, each keyed by the 32-byte change id.

## Fix

The log keeps each change in about 400 bytes: the header's fields in a compact entry, its dependencies as a slice in one node beside its clock, and each author's sequences as a list of positions. The same 81,000 one-record changes hold 30.5 MB on the heap and 53 MiB in Chromium, and 300,000 hold 113 MB and 147 MiB. Nothing on disk or on the wire changes.
