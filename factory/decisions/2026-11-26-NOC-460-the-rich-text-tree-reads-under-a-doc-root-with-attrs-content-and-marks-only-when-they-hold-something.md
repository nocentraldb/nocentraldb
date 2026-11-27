# 2026-11-26 · NOC-460 · The rich-text tree reads under a `doc` root, with `attrs`, `content` and `marks` only when they hold something

- Decided by: coding agent (NOC-460)
- Why: ProseMirror JSON has a `doc` root, which `yXmlFragmentToProsemirrorJSON` also writes, so an app passes the tree to `Node.fromJSON` as it is. A node has `type` and, when not empty, `attrs` and `content`; a text node `type`, `text` and, when not empty, `marks`; marks are sorted by name, and y-prosemirror's `--<hash>` suffix of a mark that may occur twice is dropped from its name. A `null` attribute is left out, a mark whose value is `null` or `false` is no mark, and a value Yjs embeds in text, which TipTap's mapping never writes, is not read.
- Instead of: the root fragment's children without a root; empty `attrs` and `marks` on every node, which ProseMirror fills from the schema anyway.
- Topics: rich-text, rfc-0013, reads, sdk-rust
