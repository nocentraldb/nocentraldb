# 2026-11-13 · NOC-305 · members.add matches a fingerprint from TypeScript by its digits alone

- Decided by: Carsten, 2026-11-13, on NOC-305 (option A: "The recommendations are fine with me")
- Why: the Rust `Fingerprint` has no `FromStr`, and a person may type or paste the twelve blocks with any spacing. The bridge compares the digits of the given text with the digits of the invitation's own fingerprint, ignoring spaces and grouping, as `apps/tasks-rust` does. A fingerprint whose digits differ confirms nothing, so `members.add` fails as without one, with `unknownAccount` carrying the real fingerprint.
- Instead of: a `FromStr` for `Fingerprint` in the Rust SDK, a new public name that the bridge alone would use.
- Topics: sdk-ts, membership, fingerprint
