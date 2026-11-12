---
type: internal
tickets: [NOC-175, NOC-245]
components: [sdk-rust, relay]
---
On wasm32 the Rust SDK syncs with its relay over the browser's WebSocket and carries large changes through `fetch()`, and the local relay server's upload and download URLs answer CORS for any origin. Nothing public names the browser database yet.
