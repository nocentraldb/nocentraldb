# 2026-11-08 · NOC-44 · The relay-refusal sentences of RFC 0002 and RFC 0003 follow the cutoff too

- Decided by: coding agent (NOC-44)
- Why: RFC 0002 ("On the relay") and RFC 0003 (removal, step 1) said the relay refuses a removed member's changes "from the moment" of the removal, which contradicts RFC 0008's decision M3 (the cutoff) just as directly as the points the ticket lists. Left alone, the AC "no RFC on main contradicts RFC 0008" would not hold. Likewise RFC 0003's "same or a newer generation" for person-key records became "same or a descendant generation", which RFC 0008 lists under conflict 12. Both are sentence rewrites under decisions the owner already made; they add nothing new.
- Instead of: leaving those sentences for RFC 0007's PR (#37), which aligns RFC 0007 only.
- Topics: rfc-0002, rfc-0003, rfc-0008, relay, membership
