---
type: internal
tickets: [NOC-271]
components: [relay]
---
RFC 0007's message codec and the paging of `entries` move from `ncdb-relay-server` into the relay core `ncdb-relay` (`wire`, `page`), unchanged, so the coming Lambda function speaks the same messages; the server re-exports them.
