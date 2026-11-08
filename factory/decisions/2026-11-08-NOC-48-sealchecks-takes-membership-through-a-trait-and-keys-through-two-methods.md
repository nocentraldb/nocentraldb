# 2026-11-08 · NOC-48 · SealChecks takes membership through a trait and keys through two methods

- Decided by: coding agent (NOC-48)
- Why: step 4 and the control state are out of the ticket's scope (RFC 0008, key distribution), so `SealChecks` delegates `is_member` and `add_control` to a `Membership` it is given, and learns member keys and space keys through `add_member` and `add_space_key`. Keys learned that way do not check the rejected changes again, which only a control change that passes does (D11); the type's documentation says so, and key distribution through control changes will close the gap.
- Instead of: building a membership fold here before RFC 0008's ticket, or reading keys out of control changes whose format no RFC has fixed yet.
- Topics: rfc-0002, rfc-0003, rfc-0008, log, crypto
