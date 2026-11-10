# 2026-11-10 · NOC-89 · The relay server is its own crate, `ncdb-relay-server`, on tokio and tokio-tungstenite

- Decided by: coding agent (NOC-89)
- Why: the relay core (`ncdb-relay`) is what every deployment links, AWS Lambda included, which needs no WebSocket server and no long-running runtime. The server is a separate crate with the `ncdb-relay` binary; it depends on the core and checks nothing itself. It runs on tokio =1.54.1 (already in `Cargo.lock`) with tokio-tungstenite =0.30.0 for RFC 6455, and runs every call of the relay core on tokio's blocking threads, since the SQLite backend blocks the thread that polls it. A connection answers its requests in the order they arrive; each subscription is its own task, woken by a watch of its space's head that a push on this server moves.
- Instead of: the server inside `ncdb-relay`, which would put a runtime and a WebSocket library into every Lambda build; or an HTTP framework such as axum, which adds a router this one-endpoint server does not need.
- What this does not do: a push by another process on the same SQLite database does not wake this server's subscriptions.
- Topics: relay, crates, rfc-0007
