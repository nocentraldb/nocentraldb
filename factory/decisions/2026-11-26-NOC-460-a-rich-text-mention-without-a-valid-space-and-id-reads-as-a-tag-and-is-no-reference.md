# 2026-11-26 · NOC-460 · A rich-text mention without a valid space id and object id reads as a tag and is no reference

- Decided by: pm (NOC-460)
- Why: RFC 0013's mention holds `space` and `id`, the target's ids as RFC 0009 writes them, 32 lowercase hexadecimal digits each. A node named `mention` without both, such as the one TipTap's own Mention extension writes with `id` and `label`, is no mention in that sense, so it reads as any other node outside the vocabulary, an HTML tag, and `mentions()` leaves it out. The link form `[@](ncdb:<space id>/<object id>)` then only ever carries two valid ids, which NOC-536's fromMarkdown reads back and refuses otherwise.
- Instead of: the link form with whatever the node holds, which writes `ncdb:` links that fromMarkdown must refuse and hides the node's other attributes from an agent.
- Topics: rich-text, rfc-0013, reads, mentions
