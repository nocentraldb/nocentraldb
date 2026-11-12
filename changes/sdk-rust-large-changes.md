---
type: feature
tickets: [NOC-121]
components: [sdk-rust]
---
## Problem
A change too large for one 128 KiB message, such as a large paste into a text field, stayed refused as `too_large`, and a relay's download link for another member's large change stopped syncing the space.

## Use
Nothing to call. The Rust SDK uploads a change of up to 16 MiB through the relay's upload URL and downloads one that a relay announces by link, then checks it like any other change (RFC 0007, "Large changes"). For now the URLs must be `http://`, as the local relay serves them; `https://` comes with `wss://`.
