# 2026-11-11 · NOC-190 · `SpaceStatus::lost_grants` lists the grants core's verdict marks lost

- Decided by: pm (NOC-190), after NOC-200 made the core's verdict follow RFC 0008
- Why: RFC 0008, "Concurrent add and remove of one member", defines the lost grant: "Bob's entry is a grant that did not see the removal, so Alice's removal caps it at 'none'. … Bob's entry stays accepted but without effect, and the SDK shows it as a lost grant". Since NOC-200 (#186), the core's `Verdict::Lost` marks a grant lost exactly when an accepted membership entry about the same member caps it below its role and the two are concurrent; a grant that a later entry revoked after seeing it is not lost. So the space status lists the changes the resolved membership gives `Verdict::Lost`, and the SDK adds no rule of its own.
- Instead of: the SDK's own filter that #180 first carried, which walked the control dependencies in the log to drop grants a revocation had seen, while the core still marked every lowered grant lost; NOC-200 moved that rule into the core, where every reader of the verdict gets it.
- Topics: sdk-rust, rfc-0008, rfc-0009, membership
