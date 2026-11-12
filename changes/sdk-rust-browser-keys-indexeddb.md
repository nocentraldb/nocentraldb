---
type: internal
tickets: [NOC-175, NOC-210]
components: [sdk-rust]
---
On wasm32 the Rust SDK keeps the member's keys in IndexedDB: the key record sealed with AES-GCM under a non-extractable WebCrypto key, both in the database `nocentraldb/<name>`. Nothing opens a database there yet; the browser key tests run in Chromium in browser-view.yml.
