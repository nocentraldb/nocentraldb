---
type: internal
tickets: [NOC-329]
components: [sdk-ts]
---
The TypeScript SDK's texts and the bridge's messages move into `texts/texts.json`, with when each one shows: the TypeScript SDK's as kind `ts`, with the code they are thrown with or none for the schema's errors, generating `packages/sdk/src/texts.ts`; the bridge's as errors, generating `crates/nocentraldb/src/texts/bridge.rs`. The gate `repo:texts` now fails on a text written in the TypeScript SDK's or the bridge's code too. Two bridge texts change, because a text may hold no brace but its placeholders: "a request is a map of id, call and args" and "a map value is $add or $remove with a list".
