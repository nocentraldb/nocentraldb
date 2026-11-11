---
type: internal
tickets: [NOC-200]
components: [core]
---
`ncdb-core`'s membership fold marks a grant `Lost` only when an entry about the same member that caps it below its role is concurrent with it, as RFC 0008 defines a lost grant; a grant that a later removal or demotion saw stays accepted.
