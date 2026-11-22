---
type: fix
tickets: [NOC-439]
introduced: cycle
components: [core, sdk-rust, sdk-ts]
---
## Symptom

Writing got slower with every change a space held. In the browser, a write of one object took about 6 ms in a new database, 16 ms at 30,000 objects and about 60 ms at 60,000, so 100,000 objects written one at a time would have taken well over an hour.

## When

On every write in the TypeScript SDK, which follows the status of each space a handle is held on and the durability report. In the Rust SDK, on every write while the app follows a space's status (`space.status().subscribe()`) or the durability report.

## Cause

After each write the SDK reads the status of each followed space and the durability report again. The space's status listed the changes the relay refused, the equivocations and the newest model the parked data needs by going through every change of the space's log. The durability report found the oldest change the relay had not confirmed with a query that read every row of the log table. Both cost a pass over the whole log on every write, and in a browser each row read comes from OPFS.

## Fix

The log now keeps the refused changes, the equivocations, the changes with parked data and the oldest unconfirmed change up to date as changes are written, received, confirmed and refused, so the reports read them without a pass over the log. Nothing on disk changes. In Chromium a write of one object now costs about 6 to 7.5 ms from the first object to the 100,000th.
