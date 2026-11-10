---
type: internal
tickets: [NOC-160]
components: [relay]
---
The relay server closes a connection whose opening handshake (WebSocket upgrade, `hello`, `auth`) stalls for 10 seconds at any step.
