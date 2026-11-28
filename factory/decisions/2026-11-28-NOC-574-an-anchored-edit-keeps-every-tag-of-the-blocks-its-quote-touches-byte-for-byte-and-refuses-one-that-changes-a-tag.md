# 2026-11-28 · NOC-574 · An anchored edit leaves a tag untouched when its replacement keeps it byte for byte, and refuses one that drops, changes, adds or moves a tag

- Decided by: coding agent (NOC-574)
- Why: RFC 0013 refuses an edit whose replacement drops a tag its quote contained, so an edit that keeps a tag is edited. The core reads each whole HTML element of the touched blocks' Markdown as one kept tag (tags with nothing between them as one), and each run of parts the view does not show as itself as the same tag, in order. When the replacement holds the same tags, byte for byte and in order, the diff turns the content between the tags one stretch at a time and never writes on a tag. An edit that drops, changes, adds or moves a tag, or whose quote ends inside an element's content, changes the tags and is refused with `Unsupported`, as is an edit inside a tag's content.
- Instead of: a Markdown-to-document source map that would let the core edit inside a tag, which needs the renderer rewritten; or reading a tag in the replacement as the node or mark it names, which the RFC leaves open.
- Topics: rich-text, rfc-0013, agents
