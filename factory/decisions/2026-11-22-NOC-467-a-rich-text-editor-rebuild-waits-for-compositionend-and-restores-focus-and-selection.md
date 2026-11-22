# 2026-11-22 · NOC-467 · A rich-text editor rebuild waits for compositionend, then restores the focus and the selection

- Decided by: Carsten, 2026-11-22, on NOC-467, through pm
- Why: the browser spike (NOC-455) showed a rebuild during a composition losing the committed text, keeping the uncommitted text in the document and leaving the editor without focus. A new kind of node shows a moment later instead.
- Instead of: rebuilding at once on contentError.
- Topics: rich-text, rfc-0013, sdk-ts, ime, unknown-nodes
