# 2026-11-27 · NOC-461 · The SDK returns an editor-agnostic rich-text binding, and @nocentraldb/tiptap builds and rebuilds the editor and hands each one to onEditor

- Decided by: pm, NOC-461, following RFC 0013 and NOC-538 Q4
- Why: RFC 0013 keeps the binding in @nocentraldb/sdk usable with any editor that edits a Y.Doc and puts what is TipTap's in @nocentraldb/tiptap; Q4 decided that the app gets each new editor through a callback, not which package holds the call. space.richText returns the doc, the UndoManager, the status and the composition hook; @nocentraldb/tiptap takes that binding, the app's editor options and onEditor, sets enableContentCheck itself, widens the schema and rebuilds.
- Instead of: space.richText taking onEditor and building a TipTap editor, which would make the SDK depend on TipTap.
- Topics: rich-text, rfc-0013, sdk-ts, unknown-nodes
