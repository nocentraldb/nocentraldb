---
type: internal
tickets: [NOC-178]
components: [sdk]
---
An end-to-end test runs a Chromium tab with the TypeScript SDK and a Rust device with the Rust SDK against the local relay server. The Rust device creates a space and adds the tab by its invitation once the fingerprints match. Both write, also concurrently into one object while the network is cut, and their reads converge.
