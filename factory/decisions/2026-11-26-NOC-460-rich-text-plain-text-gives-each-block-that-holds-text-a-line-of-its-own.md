# 2026-11-26 · NOC-460 · Rich-text plain text gives each block that holds text a line of its own, and a rule gives none

- Decided by: coding agent (NOC-460)
- Why: RFC 0013 reads plain text as "the text of each block, one block per line, for previews". A block that holds text is a paragraph, heading or code block, or any node with text in it; the blocks of a list item, quote, table cell or node outside the vocabulary each give a line; a hard break starts a new line, a code block keeps its lines, an empty paragraph gives an empty line, and a horizontal rule gives none.
- Instead of: one line per top-level block, which joins a list's items into one line.
- Topics: rich-text, rfc-0013, reads
