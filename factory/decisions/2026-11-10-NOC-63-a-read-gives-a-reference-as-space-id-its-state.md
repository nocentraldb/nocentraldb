# 2026-11-10 · NOC-63 · A read gives a reference as { space, id }; its state shows only in space.inspect, and subscriptions ignore it

- Decided by: Carsten, 2026-11-10, on PR #92 (RFC 0005 decisions D20 and D21)
- Why: A read object stays a function of its own space: `nc.Infer` types a reference as `{ space, id }`, and `space.inspect` gives its `status`, `"ok"`, `"notAccessible"` or `"missing"`, beside its conflicts; no view, index or computed field holds the state, since it differs from member to member. A subscription re-runs when the referencing object changes, not when the target or the member's access to it changes, because RFC 0006's notifications carry the objects a fold touched in its own space.
- Instead of: the read object carrying the state, or the SDK reading the target along with the reference (D20); subscriptions that follow the state, which need a reverse map across spaces (D21).
- Topics: rfc-0005, rfc-0009, references, sdk, subscribe
