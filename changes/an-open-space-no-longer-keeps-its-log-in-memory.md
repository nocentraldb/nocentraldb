---
type: fix
tickets: [NOC-452]
introduced: 0.1.0
components: [core, sdk-rust, sdk-ts]
---
## Symptom

Every device kept every change of every space it held in memory, as long as the database was open: about twice the bytes the relay received, 63 MB for 81,000 records written 500 to a change, growing with the log without a bound. A long-lived space would in time hold more memory than a phone or a browser tab can give it.

## When

On every device with an open database, natively and in the browser, in proportion to the bytes of the spaces' logs.

## Cause

The core's in-memory log held each change whole, its frame and its payload, also after the database's log table held the frame.

## Fix

Once the database holds a change's frame, the log in memory keeps only its header, its state and what a write needs from it; it reads the change back from the database for the rare work that needs its content, such as un-folding a change after a late revocation. Control changes stay in memory. An open space of 81,000 records written 500 to a change now holds 0.2 MB of the Rust SDK's heap instead of 63 MB, and a Chromium tab's core 19 MiB of WebAssembly memory instead of 79 MiB. A space still holds about 1.5 KB for each change it has, whatever its size: written one record to a change, the same 81,000 records hold 120 MB instead of 218 MB. Nothing on disk or on the wire changes.
