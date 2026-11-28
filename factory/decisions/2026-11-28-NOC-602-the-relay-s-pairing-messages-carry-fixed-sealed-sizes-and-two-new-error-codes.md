# 2026-11-28 · NOC-602 · The relay's pairing messages carry sealed requests of 317 bytes and answers of 398, with two new error codes

- Decided by: Carsten, 2026-11-28, on NOC-602 (option A)
- Why: RFC 0019 adds the pairing mailbox's messages to RFC 0007, `pairing-status` and `withdraw-pairing` included, so a new device that lost its connection after the person accepted still gets its answer; the draft's twelve, to which NOC-607's one-scan way adds `wait-pairing` and `send-pairing`. Requests and answers are padded inside the seal to fixed sizes, so the relay cannot tell a device from an agent or see whether a database links its own account; any other length is answered with `malformed`. The new error codes are `unknown_mailbox` and `unknown_request`.
- Instead of: fewer messages, without `pairing-status` and `withdraw-pairing`, which loses an accepted answer when the new device's connection drops (B); or no padding, which shows the relay the kind and whether a link is under way (C).
- Topics: pairing, relay, protocol, rfc-0019, rfc-0007
