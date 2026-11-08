# 2026-11-08 · NOC-22 · RFC 0002 Amendment 1: every recommendation accepted

- Decided by: Carsten
- Why: the owner accepted D10 to D12 as recommended and merged #38. D10: the change is atomic for signature, membership and decryption, and an operation on an unknown collection or field is parked on its own. D11: the member log gets a `rejected` state with a reason, checked again whenever a control change of the space arrives. D12: a change the relay refuses after the member folded it stays folded; the member records the refusal beside it.
- Instead of: the alternatives listed for each decision in the amendment in `docs/rfc/0002-change-and-log-format.md`.
- Topics: rfc-0002, log, change-format
