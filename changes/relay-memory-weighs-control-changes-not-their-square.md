---
type: internal
tickets: [NOC-492]
components: [relay]
---
The relay's per-process memory of resolved spaces weighs a space by its control changes instead of their square, up to 10,240 over all spaces (measured at about 7.7 KiB each); the NOC-427 fragment, released together with this, names that budget.
