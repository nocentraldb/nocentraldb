# 2026-11-08 · no ticket · pm merges a faithful implementation of a decided RFC

- Decided by: Carsten
- Why: the design work in the RFCs needs the owner's review, but a PR that only implements a decided RFC adds no decision of the owner's. Making each such PR wait for the owner's tick would stall the build phase. pm now checks such a PR against the RFC and the gates and ticks a new authorship box, "Agent, checked against a decided design". The PR must name the RFC sections it implements, depart from none, and bring in no public name, format or runtime dependency the RFC does not name, and change no gate (option 1A). AI_POLICY.md states the exception openly (2A). The daily note lists every PR merged this way, so the owner can read it afterwards and ask for a revert or a follow-up (3A).
- Instead of: also letting pm merge tooling, CI and refactor PRs (1B), or everything except RFCs and factory changes (1C); pm ticking "Agent with human review" (2B) or a label instead of a disclosure (2C); no safety net (3B) or a mandatory review after the merge (3C).
- Topics: factory, merge-rule, review, ai-policy
