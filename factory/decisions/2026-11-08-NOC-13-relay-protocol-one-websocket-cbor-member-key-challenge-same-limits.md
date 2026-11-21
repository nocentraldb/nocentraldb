# 2026-11-08 · NOC-13 · Relay protocol: one WebSocket of CBOR messages, a member-key challenge, and the same limits on every backend

- Decided by: Carsten (RFC 0007 decisions R1, R2, R4 and R5)
- Why: one channel, one authentication and one encoding work in every browser and on API Gateway (R1); a challenge signed with the member key lets no new party learn who the members are, leaves no token to steal and replay, and uses the key the SDK already holds (R2); a member never behaves differently by backend (R4); 16 MiB is large enough for rich-text pastes with images inlined and small enough that one change cannot block a member's pull (R5).
- Instead of: HTTP requests with server-sent events, or gRPC with a proxy for browsers (R1); bearer tokens from an identity service such as Cognito (R2); limits per backend (R4); 4 MiB, or no limit with multipart uploads (R5).
- Topics: relay, rfc-0007, protocol, wire

The rules stand in `docs/rfc/0007-relay.md`, "Transport and encoding", "Opening a connection" and "Large changes". A developer's own adapter may put its own authentication, such as Cognito, in front of the connection; our AWS backend does not. Later decisions changed the encoding and the numbers, not the rule: every message travels as base64 in one text frame of at most 23 KiB ([NOC-338](2026-11-15-NOC-338-every-message-travels-as-base64-in-one-text-frame-of-at-most-32-kb.md)), and message fields take integer keys ([NOC-98](2026-11-10-NOC-98-relay-message-fields-take-keys-in-rfc-0007s-order.md)).
