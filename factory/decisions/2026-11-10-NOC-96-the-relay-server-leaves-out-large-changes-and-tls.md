# 2026-11-10 · NOC-96 · The relay server leaves out large changes and TLS

- Decided by: coding agent (NOC-96), carrying over NOC-89's safe subset without its keys part
- Why: NOC-96 serves `keys`, `person-keys` and `put-person-keys`, which NOC-89 left out. What NOC-89 decided for the rest stands: the largest frame the server accepts, announced as limit 3 of `challenge`, is 127 KiB, the largest whose entry still fits into one 128 KiB `entries` message, so every entry travels inline; a larger frame is `too_large`, and so are `upload` and an upload reference in a push. The binary serves plain `ws://`, by default on 127.0.0.1; RFC 0007's TLS is left to a proxy in front of it.
- Instead of: `unavailable`, which tells the member to retry with backoff forever; or a `download` link the local server has nowhere to serve from.
- What this does not do: a change above 127 KiB stays on its member's device until a relay carries large changes.
- Topics: relay, rfc-0007, safe-subset
- Supersedes: 2026-11-10-NOC-89-the-relay-server-leaves-out-large-changes-keys-and-person-keys.md
