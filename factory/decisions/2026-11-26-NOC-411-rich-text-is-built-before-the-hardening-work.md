# 2026-11-26 · NOC-411 · Rich text is built before the hardening work

- Decided by: Carsten, 2026-11-26, in the pm session
- Why: a consumer using NoCentralDB is to run fully on it first, then come that consumer's desktop app and meeting agent, then the move of the owner's own data into it, then the rest; its notes need rich text, so RFC 0013's build steps go ahead of the hardening work that until now came first until about April 2027.
- Instead of: building rich text after the hardening phase.
- What this does not change: a release carries rich text only once the Yrs fixes ship (NOC-468); the dates and deferrals in the NOC-181 and NOC-485 entries stay as they are.
- Topics: rich-text, roadmap, priorities
