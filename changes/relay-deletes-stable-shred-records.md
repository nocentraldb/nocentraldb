---
type: internal
tickets: [NOC-113]
components: [relay]
---
The relay deletes a shredded person key's record once the shred is stable in its log (RFC 0008's earlier rule; since NOC-229 a shred is final when accepted, RFC 0008, "A shred is final when accepted"), using the core's stability with each author's gapless run; until then the record stays stored and no new record of its id is accepted.
