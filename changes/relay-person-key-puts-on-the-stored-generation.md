---
type: internal
tickets: [NOC-118]
components: [relay]
---
The relay's backend stores person-key records only on the generation the core checked them against, so two concurrent `put-person-keys` of one id can no longer take the stored record back to a generation that does not descend from it; the conformance suite holds every backend to it.
