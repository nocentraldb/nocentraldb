---
type: security
tickets: [NOC-390]
components: [relay]
---
## Description
The relay kept serving a member whose add a later removal excluded, for example an accomplice that a stolen admin device added before the owner removed that device with a cutoff below the add. The relay accepted the member's uploads, served it pulls, listed the space for it, and still admitted changes it had written before. Now, when a control change drops a member out of `resolve` of the relay's log, the relay rewrites the member's row as no current member that admits no change, so it refuses all of these. Both the local and the AWS relay get the fix. A row that went stale before the fix stays stale until the next control change in its space.

## Advisory
pending
