# 2026-11-28 · NOC-574 · An anchored edit refuses with `InvalidValue` an `ncdb:` link that is no mention's form, unless the Markdown it replaces holds the same link

- Decided by: coding agent (NOC-574)
- Why: the owner decided (NOC-612) that anchored edits refuse a malformed `ncdb:` link the way a first document from Markdown does (NOC-536): any case of the scheme, and a mention's link with a name as its text. A link the document already holds reads back in the quote's Markdown, and a replacement that keeps it would otherwise be refused for something the agent did not write, so the edit compares the replacement's malformed links with the quote's, destination by destination, and refuses only one it adds. A replacement that keeps such a link keeps a plain link, as the document already has it. Blocks added with `after` or `append` replace nothing, so any such link in them is refused.
- Instead of: refusing every malformed `ncdb:` link in a replacement, which would block every edit of a block that holds one.
- Topics: rich-text, rfc-0013, agents
