---
type: feature
tickets: [NOC-496]
components: [sdk-rust, sdk-ts]
---
## Problem
A relay protected by a short-lived token in its URL stopped syncing once the token expired: the SDK reconnected with the URL it was opened with, every reconnect was refused, and the app was not told why. Only reopening the database with a fresh URL helped.

## Use
Give the relay as a function that returns its URL. The SDK calls it before each connect, so the next connect carries a fresh token.

```ts
const db = await nc.open({
  model,
  lock,
  relay: async () => `wss://relay.example.com/prod?token=${await freshToken()}`,
});
```

In Rust, `OpenOptions::new(lock).relay_with(|| async { Ok::<_, MyError>(fresh_url().await?) })`, beside `relay(url)`.

While the function throws or gives no `ws://` or `wss://` URL, the sync status reports `relay_url_failed` and the SDK calls it again after a backoff. In the Rust SDK, a connection the relay refuses with HTTP 401 or 403 shows as `connect_refused`. A browser does not tell the SDK why a WebSocket failed, so in the TypeScript SDK a refused connection shows as offline, like an unreachable relay.
