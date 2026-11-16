---
type: internal
tickets: [NOC-336]
components: [sdk-ts]
---
The TypeScript SDK's build no longer empties `packages/sdk/dist/` first. It builds beside it and moves each file in with a rename, so a page served from `dist/` during a build reads every file whole, as when the sdk-browser e2e files run in parallel. The e2e files likewise rename the relay server and the Rust peer into place, instead of copying over a binary another file may be running.
