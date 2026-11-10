---
type: internal
tickets: [NOC-96]
components: [core, relay]
---
The core reads person-key and shred entries and checks a shred's second-account sign-off in `resolve`; the relay accepts shreds, stores person-key records and answers `keys`, `person-keys` and `put-person-keys`.
