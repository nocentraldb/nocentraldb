---
type: internal
tickets: [NOC-103]
components: [relay]
---
The relay server bounds each connection's outgoing queue, closes a member that takes nothing for 60 seconds, and binds only a loopback address, with TLS from a proxy on the same host (see its README).
