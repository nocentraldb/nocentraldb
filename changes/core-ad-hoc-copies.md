---
type: internal
tickets: [NOC-318]
components: [core]
---
The core keeps, on the parent only, which object each copy in an ad-hoc space came from, in a device-local table that a rebuild keeps and that never syncs: the first step towards ad-hoc spaces for sub-agents.
