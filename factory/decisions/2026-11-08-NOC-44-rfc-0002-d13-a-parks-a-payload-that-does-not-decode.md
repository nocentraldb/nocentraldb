# 2026-11-08 · NOC-44 · RFC 0002 D13 is A: a payload that decrypts but does not decode is parked as "unknown payload format"

- Decided by: Carsten
- Why: the owner signed off RFC 0002 Amendment 2 by merging #54, with its recommendation: A is the only option that keeps convergence without freezing the payload profile and the depth limit, and it needs no new state. Amendment 1 (D10 to D12) was signed off the same way by merging #38. NOC-44 rewrites the RFC's status line, the amendments' intros and the decisions' "open" lines as decided.
- Instead of: B, rejected as "undecodable payload", which no control change can check again; or C, split by cause, which makes every relaxation of the payload profile a new header format.
- Topics: rfc-0002, log, change-format
