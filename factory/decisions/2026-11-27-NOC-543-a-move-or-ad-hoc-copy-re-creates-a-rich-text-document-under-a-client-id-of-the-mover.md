# 2026-11-27 · NOC-543 · A move or an ad-hoc copy re-creates a rich-text document from its tree under a client id of the mover, and carrying it as it is comes later as an option per move

- Decided by: Carsten, 2026-11-27, NOC-543
- Why: RFC 0013 binds every Yjs client id to the member that wrote it, so the mover cannot write other members' items into another space as they are; re-creating the document as one ordinary rich-text update keeps that guarantee and needs no change to the format or the fold. Who wrote which part, and the document's history, stay in the old space's log, as RFC 0016 keeps every other field's history there. An app may later want to choose per move, so carrying the document as it is comes later as an option (NOC-559).
- Instead of: carrying the document as it is, with its item ids, client ids and nonces, which changes the format and the fold and weakens the rule that a client id names its author; re-creating it from its Markdown, which drops the nodes and attributes Markdown does not show; refusing to move or copy an object whose rich-text field was written.
- Topics: rich-text, move, ad-hoc-spaces, rfc-0013, rfc-0016
