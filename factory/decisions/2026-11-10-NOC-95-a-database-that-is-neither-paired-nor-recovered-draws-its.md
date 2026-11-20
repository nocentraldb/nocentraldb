# 2026-11-10 · NOC-95 · A database that is neither paired nor recovered draws its own account key, and agent() marks it an agent

- Decided by: Carsten, 2026-11-10, on NOC-95 (RFC 0009 decision S20, RFC 0003 decision 17)
- Why: Each database draws its own account key once and keeps it next to the member key, so a person's devices are unrelated accounts until RFC 0010's pairing or linking joins them; `agent()` marks the member as an agent in its certificate, so a space adds it as a writer when no role is given (RFC 0008, M14).
- Instead of: the app passing the account key, which puts a key in the app's hands (S20 B); no space until an accounts RFC exists, which blocks every space (S20 C).
- Topics: rfc-0009, rfc-0003, accounts, keys, sdk-rust
