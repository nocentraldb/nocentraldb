# 2026-11-22 · NOC-467 · The rich-text binding keeps one UndoManager for its Y.Doc across editor rebuilds, so a rebuild keeps the undo history

- Decided by: Carsten, 2026-11-22, on NOC-467, through pm
- Why: the browser spike (NOC-455) showed undo working after a rebuild when one UndoManager is passed to each editor, so the cost RFC 0013 stated for a rebuild is not needed. Apps pass `body.undoManager` to TipTap's Collaboration extension.
- Instead of: each editor's own UndoManager, losing the undo history at every rebuild.
- Topics: rich-text, rfc-0013, sdk-ts, undo, unknown-nodes
