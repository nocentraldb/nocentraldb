# 2026-11-11 · NOC-219 · The spaceLeft notice names both limits: the member keeps its copy, and the others write under a key it holds until an admin rotates

- Decided by: Carsten (2026-11-11, on NOC-219, option A)
- Why: RFC 0009 named the notice code `spaceLeft` for `space.leave()` without a text, and a notice states plainly what the call does not do. The text is "This member keeps its copy of everything it received before now. Until an admin rotates the space key, the others write under a key it still holds." A member that leaves cannot rotate the key itself (RFC 0008), so the space stays exposed until an admin rotates.
- Instead of: "This member keeps its copy of everything it received before now." (B), shorter and in the style of `memberRemoved`, but it leaves out the exposure.
- Topics: notices, texts, members, rfc-0009
