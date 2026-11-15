# 2026-11-15 · NOC-320 · A sub-agent's invitation is version 3 of the invitation

- Decided by: Carsten, 2026-11-15, on NOC-320 (Q1 a)
- Why: a sub-agent opens its database without a certificate; its parent signs one when it adds it (RFC 0003, "Sub-agents get an ad-hoc space"; RFC 0009, "Sub-agents and ad-hoc spaces"). Version 1 carries a certificate, and version 2 means "pair me into an account", which a sub-agent never does. Version 3, under the prefix `ncdb-invite-1:`, is the version byte `3`, the member's Ed25519 public key and its X25519 public key: 65 bytes. Pairing refuses it by its version; only a parent's `members.add` takes it. Its vectors go into `docs/rfc/vectors/0003/`, and its layout into RFC 0003, "Invitation", in the slice that builds it.
- Instead of: version 2 with kind 3 allowed, which would make a sub-agent's invitation one that pairing must refuse by its kind rather than its version.
- Topics: sub-agents, invitation, format
