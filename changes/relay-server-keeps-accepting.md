---
type: internal
tickets: [NOC-192]
components: [relay]
---
The relay server keeps running when it cannot accept a connection, for example when it runs out of file descriptors: it logs the error and accepts again after a pause of up to one second.
