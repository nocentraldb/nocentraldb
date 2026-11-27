# 2026-11-27 · NOC-462 · An anchored edit reads back only the top-level blocks its quote touches, and is refused when they do not read back as the document holds them

- Decided by: coding agent (NOC-462)
- Why: the core renders the agent's version as Markdown, finds the quote there, and reads the top-level blocks the quote touches back from that Markdown with pulldown-cmark (pinned, under the cfg `ncdb_rich_text`). When they read back as what the document holds, as far as Markdown shows it, the replacement's blocks are compared with them and only the difference is written; parts Markdown does not show, such as underline or a link's target, are never touched. When they do not, for a node or mark outside the vocabulary (an HTML tag), HTML in the replacement, or a node that would split a text, the edit is refused and nothing is written, since the core would otherwise write more than the agent changed. Blocks the quote does not touch take no part, so a tag elsewhere in the document does not stop an edit.
- Instead of: reading the whole document back, which a single tag anywhere would stop; or a source map from every Markdown offset to a document position, which needs the renderer rewritten.
- Topics: rich-text, rfc-0013, agents, yjs

Editing inside tags, and nodes inside a run of text, are follow-up work; the refusal's public name waits for NOC-565.
