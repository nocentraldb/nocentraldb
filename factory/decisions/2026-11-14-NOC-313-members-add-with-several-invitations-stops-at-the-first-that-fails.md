# 2026-11-14 · NOC-313 · members.add with several invitations stops at the first that fails

- Decided by: Carsten, 2026-11-14, on NOC-313 (A)
- Why: the TypeScript `members.add` takes a list of invitations (RFC 0009, "Invitations and members"), and the Rust SDK adds one member per change. The SDK adds the members in turn and stops at the first invitation that fails, with that invitation's error; the members added before it stay, each invitation is checked only when its turn comes, and a call that succeeds returns the last invitation's notice. `Members.add`'s doc comment and RFC 0009 say so, and a bridge test pins it.
- Instead of: all or none (option B), which checks every invitation first and writes all of them in one change; it needs the Rust SDK to add several members in one change, which RFC 0009 does not name.
- Topics: sdk-ts, membership, rfc-0009
