# 2026-11-26 · NOC-460 · The rich-text Markdown escapes every character that would read as markup, so it reads back as the document's text

- Decided by: coding agent (NOC-460)
- Why: an agent quotes the Markdown it read (RFC 0013, "An agent: anchored edits") and NOC-536 reads Markdown back into a document, so text a person typed must never read as markup. The core escapes `\`, `` ` ``, `*`, `_`, `[`, `]`, `<` and `~` everywhere, `&` where it would start a character reference, `|` in a table cell, and at the start of a line `#`, `>`, `+`, `-`, `=` and the `.` or `)` after a number. Whitespace at the edge of a mark moves outside it; of marks that open together the longest-running opens first; links nest outside other marks and code inside. Lists are tight, a second list in a row takes the other marker (`*`, `)`), and a table's first row is its header.
- Instead of: escaping only where a character is ambiguous in its context, which reads more cleanly but makes the rules for NOC-462 and NOC-536 depend on the context of every character.
- Topics: rich-text, rfc-0013, reads, agents
