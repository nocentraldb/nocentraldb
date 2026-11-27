# 2026-11-26 · NOC-461 · The rich-text binding writes at once when the page is hidden or the binding closes, also during an IME composition

- Decided by: pm, NOC-461
- Why: NOC-467 holds a write during a composition for at most 2 s; leaving the page ends that hold, since waiting for compositionend there would lose the text the person committed.
- Instead of: holding the write until compositionend also on hide and close, which loses the edits when the tab goes away.
- Topics: rich-text, rfc-0013, sdk-ts, ime
