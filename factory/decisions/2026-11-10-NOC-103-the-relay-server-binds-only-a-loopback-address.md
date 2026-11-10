# 2026-11-10 · NOC-103 · The relay server binds only a loopback address, until the owner decides how it gets TLS

- Decided by: coding agent (NOC-103), as a safe subset of RFC 0007's "Transport and encoding"
- Why: RFC 0007 requires TLS for every connection ("one WebSocket connection (RFC 6455) with TLS") but does not say whether the server terminates it or a proxy in front of it does. The server serves plain `ws://` (NOC-89, NOC-96), so on any address beyond the loopback it would serve the protocol without TLS. `Server::bind`, and with it `ncdb-relay --listen`, therefore refuses an address that is not a loopback address with `InvalidInput`, before it binds anything. A TLS proxy on the same host reaches it on the loopback; the crate's README shows one. Built-in TLS, or an explicit option that allows a non-loopback address behind a proxy on another host, is a choice developers would see, so it goes to the owner through pm.
- Instead of: built-in TLS or a new command-line option chosen without the owner; or the old behaviour, which bound any address `--listen` named and served it in plain text.
- What this does not do: a TLS proxy on another host, such as a load balancer, cannot reach the server; and nothing checks that a proxy on the loopback really terminates TLS.
- Topics: relay, rfc-0007, safe-subset, tls
