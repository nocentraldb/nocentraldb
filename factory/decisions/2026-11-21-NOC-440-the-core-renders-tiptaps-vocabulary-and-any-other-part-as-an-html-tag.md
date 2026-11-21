# 2026-11-21 · NOC-440 · The core renders rich text in TipTap's standard vocabulary as Markdown, any other part as an HTML tag, and the vocabulary is not part of the merge rule

- Decided by: Carsten, 2026-11-21, on RFC 0013's D4, through pm
- Why: agents and the Rust SDK see everything, unknown parts included, as tags they can quote around, so D5's Markdown quotes work fully; no lock rule is needed; and since the vocabulary changes what members read, never the state they converge on, a new standard node needs no migration. Whether every client keeps editing parts it cannot interpret is the editor binding's job, by its three measures, under every option.
- Instead of: the vocabulary declared in the lock, where every added node is a refused change of an existing field's configuration; the app owning the vocabulary with plain text for agents and the Rust SDK; a fixed vocabulary that shows unknown parts as bare text.
- Topics: rich-text, rfc-0013, agents, reads
