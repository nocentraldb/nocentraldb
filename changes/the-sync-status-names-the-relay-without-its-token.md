---
type: fix
tickets: [NOC-495]
introduced: cycle
components: [sdk-ts]
---
## Symptom
`db.sync.current().relay` held the relay URL as `nc.open` got it, query string and fragment included. An app that logged or reported its sync status logged the token a protected relay takes in its query string.

## When
In the TypeScript SDK, with a relay URL such as `wss://relay.example.com/prod?token=…`, the way the relay's CDK construct protects its endpoint.

## Cause
The SDK copied the relay option into the sync status unchanged.

## Fix
`SyncStatus.relay` is the relay URL without its query string and fragment, such as `wss://relay.example.com/prod`. The SDK still connects with the whole URL. The Rust SDK shows the relay URL nowhere: not in its sync status, its errors or `Debug`.
