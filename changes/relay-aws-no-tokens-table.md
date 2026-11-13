---
type: internal
tickets: [NOC-303]
components: [relay]
---
The relay's AWS backend keeps no `tokens` table: the backend interface's token-URL methods form their own trait, `TokenUrls`, which only backends that serve their own URLs implement, and the conformance suite tests them only there.
