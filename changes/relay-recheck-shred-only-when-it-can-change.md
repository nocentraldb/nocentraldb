---
type: internal
tickets: [NOC-207]
components: [relay]
---
The relay re-checks a waiting shred's stability only after a control change or a change of a member that holds or held the admin role, so writers' and readers' sealed changes no longer cost a log read and a `resolve`.
