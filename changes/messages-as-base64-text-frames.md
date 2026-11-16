---
type: fix
tickets: [NOC-338]
introduced: cycle
components: [sdk-rust, sdk-ts, relay]
---
## Symptom

No device could sync with the relay deployed on AWS. Every connection closed about 50 ms after it opened, and `sync` ended with "still waiting for the connection, now Offline".

## When

Always, against the AWS relay. The local relay server was not affected.

## Cause

The SDK sent each message of the relay protocol as a binary WebSocket frame, and API Gateway's WebSocket APIs close a connection that sends one. API Gateway also limits a frame to 32 KB.

## Fix

Every message travels as base64 in one text frame, between the SDK and every relay, and no message is larger than 23 KiB, so its base64 fits one 32 KB frame. A change larger than 22 KiB goes up and comes down through an upload, as larger changes already did.
