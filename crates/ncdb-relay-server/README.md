# ncdb-relay-server

The NoCentralDB relay server: the protocol of [RFC 0007](../../docs/rfc/0007-relay.md) over WebSocket, in front of the relay core of `ncdb-relay`, on its SQLite backend. The `ncdb-relay` binary runs it locally.

```text
ncdb-relay --database <path> [--listen <address>] [--allow-account <hex>]...
```

- `--database` is the SQLite file that holds the spaces' logs. The relay id lives next to it, in `<path>.relay-id`, and is drawn on the first start.
- `--listen` is the address to serve on, by default `127.0.0.1:7700`. It must be a loopback address (see [TLS](#tls)).
- `--allow-account` names, as 64 hex digits, an account key whose members may create spaces (RFC 0007, decision R6). Give it once per account.

It prints one line, `ncdb-relay listening on ws://<address>`, and serves until it is stopped.

## TLS

RFC 0007 requires TLS on every connection. This server does not terminate TLS: it serves plain `ws://`, and so it refuses to listen on any address but a loopback address (`127.0.0.1`, `::1`, or a name such as `localhost` that resolves only to them). A member on the same machine reaches it directly; a member on the network reaches it through a TLS proxy on the same host, which forwards to the loopback address.

With [Caddy](https://caddyserver.com), for example, which gets a certificate for the name on its own and forwards WebSocket connections as they are, a `Caddyfile` of three lines is enough:

```text
relay.example.com {
	reverse_proxy 127.0.0.1:7700
}
```

Members then connect to `wss://relay.example.com`. Any proxy that terminates TLS and forwards WebSocket upgrades works the same way, such as nginx with `proxy_http_version 1.1` and the `Upgrade` and `Connection` headers passed on.

**What this does not do.** A proxy on another host, such as a cloud load balancer, cannot reach the server, since it listens only on the loopback. Nothing checks that the proxy in front of it really uses TLS: the server only makes sure that nothing beyond the host reaches it without one. The proxy sees every connection's metadata, as the server does (RFC 0007, "What the relay sees"), but no content: changes stay encrypted end to end.

## A member that reads slowly

Each connection holds at most 32 outgoing messages. A subscription reads the log only as fast as its member takes the entries, so a slow member gets every entry, in order, and the server does not buffer a space for it. A connection that takes no message for 60 seconds is closed; the member reconnects and resubscribes from its cursor (RFC 0007, "Liveness"), and misses nothing.

## A peer that stops in the handshake

Each step of the opening handshake (RFC 0007, "Opening a connection") must finish within 10 seconds of its start: the WebSocket upgrade, `hello`, and `auth` after the challenge. When a step takes longer, the server closes the connection, so a peer that stops halfway holds a task for at most 30 seconds. WebSocket pings do not extend a step. Once `welcome` is sent, the deadline no longer applies; an idle member stays connected. The server does not limit how many connections are open at once, so many peers that each stop in the handshake still hold one task each until their deadline passes.

## What it does not do yet

- Large changes (RFC 0007, "Large changes"): the largest frame it accepts fits into one message, and `upload` answers `too_large`.
- Its subscriptions wake only on pushes to this server process, not on another process that writes to the same database.
