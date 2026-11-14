---
type: internal
tickets: [NOC-278]
components: [relay]
---
The AWS Emulator's missing WebSocket data plane is pinned by a test: it fails the day the emulator answers `GoneException`, and the local tests that stand in for delivery to a gone subscriber say so. The coverage check reports whether the emulator accepts a WebSocket upgrade.
