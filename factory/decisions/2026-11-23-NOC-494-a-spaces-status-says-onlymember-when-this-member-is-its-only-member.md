# 2026-11-23 · NOC-494 · A space's status says onlyMember (Rust only_member) when this member is the space's only member, of any kind

- Decided by: the coding agent on NOC-494, on pm's brief; the public name awaits the owner's review in the PR
- Why: RFC 0010 ("What it restores") promises that the space's status says when losing this device loses the space. The status is computed on the member's own copy, so the only member can only be this member; a boolean next to `exposed` and `waitingForKey` fits RFC 0009's other status values, and "only member" is RFC 0010's own wording. It is true for an agent that is a space's only member too, since losing its storage loses the space as well. A removed member reads `false`.
- Instead of: counting only devices, as RFC 0010's sentence names them, which would leave an agent that is a space's only member unwarned; a notice, which a report value already covers; a member count, which tells the app less plainly what is at stake; or the names `soleMember` and `lastMember` (`lastAdmin` is an error about leaving, not a state).
- Topics: rfc-0009, rfc-0010, space-status, recovery, sdk-rust, sdk-ts
