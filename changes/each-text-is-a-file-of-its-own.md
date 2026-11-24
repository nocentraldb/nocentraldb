---
type: internal
tickets: [NOC-512]
components: [core, sdk-rust, sdk-ts, cli]
---
Each developer-facing text is a file of its own under `texts/`, and the generated text modules list their texts by a hash of their ids, so two pull requests that add texts no longer conflict; no text changes.
