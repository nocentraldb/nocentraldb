---
type: fix
tickets: [NOC-422]
introduced: cycle
components: [core, sdk-rust, sdk-ts]
---
## Symptom

A large write took memory far beyond its size: a write of 40,000 records, a change of 15.5 MB, peaked at 1.1 GB on the writing device and at 610 MB on a device that received it, and grew a Chromium tab by about 1 GB. A change near the relay's 16 MiB limit could run a browser tab or a phone out of memory. After a new device caught up a burst, SQLite's write-ahead log also stayed at about the size of the database until the database closed.

## When

On every write and every received change, in proportion to the change's size; it showed above a few MB a change.

## Cause

The core turned a whole change into one tree of values to seal or open it, kept each operation's value as such a tree, which takes about twenty times its bytes, and held the change's bytes several times over: the payload the writer sealed, a second decoding of the change for the log, and a copy of it. In the browser, the bridge also held a write's records three times. SQLite keeps its write-ahead log at the largest size a transaction grew it to.

## Fix

The core seals and opens a change one operation at a time, keeps each operation's value as the bytes the change carries, and holds a change's bytes once; the browser's bridge holds a write's records once. The same write of 15.5 MB now peaks at 231 MB on the writer and 161 MB on a receiving device, and grows a Chromium tab by about 485 MB. The write-ahead log shrinks back to 4 MiB from the first write after a checkpoint. Nothing on disk or on the wire changes.
