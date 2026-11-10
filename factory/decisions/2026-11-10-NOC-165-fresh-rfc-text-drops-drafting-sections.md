# 2026-11-10 · NOC-165 · A fresh RFC text drops its drafting sections, and other RFCs' references follow in their own rewrites

- Decided by: coding agent (NOC-165), for pm to check against the owner's wish on NOC-165
- Why: the owner wants each RFC to read as if just written. Besides "Decisions", RFC 0002's "Decisions this RFC follows" and "Conflicts with GENESIS.md and RFC 0001" only recorded how the text got there; each of their points already stood in, or now moves into, its section (the reading of principle 2 into "Versioning", the meaning of "envelope" into "Terms"). A former decision with no section of its own gets one (RFC 0002's "Padding"). "What RFC 0003 and RFC 0004 need from this RFC" stays: it is the interface other RFCs rely on, not bookkeeping. Other RFCs that name RFC 0002's decision ids in prose (RFC 0003, 0004, 0008, 0009) keep them until their own rewrite in this ticket, so this PR does not collide with the open PRs on those files; only a link to a removed anchor is fixed at once.
- Instead of: keeping "Conflicts" as a section of readings; or editing every other RFC's prose reference in RFC 0002's PR.
- Topics: rfc, docs
