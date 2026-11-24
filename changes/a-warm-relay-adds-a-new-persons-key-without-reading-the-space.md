---
type: internal
tickets: [NOC-497]
components: [core, relay]
---
A warm relay adds a person-key entry to a remembered space's resolve with the new `Resolver::add`, instead of resolving every control change again, and keeps the change it appended; the NOC-427 fragment, released together with this, names the new cost.
