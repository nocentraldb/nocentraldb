---
type: internal
tickets: [NOC-293]
components: [relay]
---
The relay's orphan sweep on AWS runs every hour in runs that stop before the function's timeout and resume where the last stopped, so a pass finishes however many frames the bucket holds; an alarm fires when no pass completed in two days.
