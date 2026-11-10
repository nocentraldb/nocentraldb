---
type: internal
tickets: [NOC-164]
components: [relay]
---
The relay server holds at most 512 connections open at once, at most 64 of them in the opening handshake, and closes a connection above either limit as soon as it accepts it.
