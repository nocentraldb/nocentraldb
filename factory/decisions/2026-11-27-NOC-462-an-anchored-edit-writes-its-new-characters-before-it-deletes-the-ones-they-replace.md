# 2026-11-27 · NOC-462 · An anchored edit writes its new characters before it deletes the ones they replace, and only adds marks to whitespace

- Decided by: coding agent (NOC-462)
- Why: text is compared by its common start and end; the characters between are replaced. Written first, the new characters sit between the kept start and the old characters, so a person who typed next to or inside the old characters since the read keeps their place after the new ones ("We ship on Mon-day."), instead of an order Yjs sets by client id. Whitespace gains a mark the replacement gives it, but never loses emphasis or a link, since Markdown moves whitespace at the edge of a mark outside it and cannot show it there.
- Instead of: deleting first, which leaves the new characters next to deleted ones and orders them against the person's typing by client id; removing marks from whitespace whenever the Markdown does not show them, which would strip a trailing space of its bold at every edit.
- Topics: rich-text, rfc-0013, agents, yjs
