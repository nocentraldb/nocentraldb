---
type: internal
tickets: [NOC-337]
components: [sdk-rust]
---
The relay tests of a device that syncs while it waits to be paired wait until it holds the account space too before it completes the pairing, so they no longer fail under load.
