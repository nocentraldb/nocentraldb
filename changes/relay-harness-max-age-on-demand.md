---
type: internal
tickets: [NOC-309]
components: [relay]
---
The relay's local gateway can end its connections' maximum age on demand, so the test of the max-age close no longer races its own answers against a 400 ms timer; a second test checks the timer itself on a connection that sends nothing.
