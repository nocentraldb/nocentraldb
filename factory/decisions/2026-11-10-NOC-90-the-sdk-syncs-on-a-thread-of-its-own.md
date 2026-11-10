# 2026-11-10 · NOC-90 · The Rust SDK syncs on a thread of its own, with its own tokio runtime and tokio-tungstenite

- Decided by: coding agent (NOC-90)
- Why: RFC 0009 says the SDK syncs in the background and the app never sees the protocol, and every SDK call finishes on the device. A database opened with a relay starts one thread, `nocentraldb-sync`, that runs a current-thread tokio runtime (tokio =1.54.1) and one WebSocket connection (tokio-tungstenite =0.30.0, the versions the relay server already pins), so an app needs no runtime of its own and any executor can poll the SDK's futures. The thread holds only a weak reference to the database and takes its lock only between messages, never across an await; it ends when the database is dropped. The member signs the relay's challenge with `ncdb_relay::auth::digest`, the same function the relay verifies with.
- Instead of: spawning on the app's tokio runtime, which forces tokio on every app; or a sync call the app drives, which RFC 0009 does not name.
- What this does not do: it runs natively only, like the rest of this crate; the browser needs its own transport.
- Topics: sdk-rust, relay, crates, rfc-0009
