# 2026-11-26 · NOC-459 · A rich-text attribute value that a rejected change overwrote is lost

- Decided by: coding agent (NOC-459)
- Why: RFC 0013 says an attribute the rejected change set ends unset, since Yjs deleted the older value when the newer arrived. The older value's item is then deleted without any accepted delete set naming it, which is the outcome rule's `lost`, so `$conflicts` names the field and `space.inspect` gives the value back, the same way on every member.
- Instead of: a new outcome for values a rejected change displaced, which RFC 0013's outcome rule does not have.
- Topics: rich-text, rfc-0013, un-folding, outcomes
