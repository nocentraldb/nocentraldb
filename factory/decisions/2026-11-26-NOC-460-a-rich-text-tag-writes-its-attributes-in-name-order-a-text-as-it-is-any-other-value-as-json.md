# 2026-11-26 · NOC-460 · A rich-text tag writes its attributes in name order, a text as it is and any other value as JSON

- Decided by: coding agent (NOC-460)
- Why: RFC 0013 renders a node or mark outside the vocabulary as an HTML tag with its name and attributes; Yrs keeps attributes in hash maps, so only a fixed order keeps the read a function of the state (C7). Values are written in double quotes with `&` and `"` as character references (`|` too in a table cell); `null` leaves an attribute out. A node that holds text reads as `<name …>text</name>` on one line, one that holds blocks with a blank line after the opening and before the closing tag, so Markdown reads its content as Markdown. A heading level outside 1 to 6, an ordered list's start that is no whole number, and a link without a text `href` read as tags too.
- Instead of: the attributes in the order Yrs returns them, which differs from copy to copy; dropping attributes that are no text, which hides them from an agent.
- Topics: rich-text, rfc-0013, reads, convergence
