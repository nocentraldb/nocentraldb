---
type: internal
tickets: [NOC-290]
components: [relay]
---
The relay on AWS sweeps orphaned frames daily: an EventBridge rule invokes the function with the sweep's own input, which deletes frames older than a day that no change holds; a failed sweep raises the error alarm.
