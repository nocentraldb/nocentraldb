# 2026-11-26 · NOC-459 · A rich-text placeholder gives way to every write the field folds that adds one of its ids

- Decided by: pm, NOC-459
- Why: the document must read as if the rejected change had never been written, so a write that parked as "malformed arguments" because the rejected change gave its ids other content folds once that change is rejected, and the placeholder is not placed. The field is folded in stamp order with its writes first, the ones parked as malformed arguments included, then the placeholders; a placeholder that shares an id with a write, whatever their content, is left out, so it never deletes what a write added. Of two placeholders that give one id different content, the earlier in stamp order keeps it, as for writes.
- Instead of: placeholders that keep their ids in stamp order like writes, which keeps a write parked for a change no member accepts.
- Topics: rich-text, rfc-0013, un-folding, convergence

This answers the open point of the NOC-457 entry on folding again in stamp order: un-folding now checks again the writes that parked because of the rejected change.
