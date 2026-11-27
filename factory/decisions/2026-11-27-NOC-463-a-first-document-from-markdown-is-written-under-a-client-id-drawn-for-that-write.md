# 2026-11-27 · NOC-463 · A first document from Markdown is written under a client id of its author drawn for that one write, not through a lease

- Decided by: coding agent (NOC-463)
- Why: `RichText::from_markdown` writes once into a field that holds no content, inside a write that sees the view only to read it. A client id derived from the member and a nonce drawn for the write (RFC 0013, "Client ids name their author") is checked on fold like any other, and since no document holds an item of it, its clocks start at 0 and no item id can repeat. A field gets at most one such id per first document, so the reuse a lease gives saves next to nothing here.
- Instead of: leasing a client id for the write, which needs the view mutably inside the write's encoding and a release after it, for a client id that writes once.
- Topics: rich-text, rfc-0013, yjs, sdk-rust
