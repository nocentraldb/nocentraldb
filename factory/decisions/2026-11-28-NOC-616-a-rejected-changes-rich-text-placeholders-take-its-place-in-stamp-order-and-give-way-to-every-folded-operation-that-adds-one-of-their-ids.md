# 2026-11-28 · NOC-616, NOC-605 · A rejected change's rich-text placeholders take its place in stamp order, and give way to every folded operation that adds one of their ids

- Decided by: pm, NOC-616, carrying out Carsten's decision on NOC-611
- Why: with the field folded in stamp order and nothing pending, an edit placed next to a rejected change's text needs its placeholders before it; placeholders after every other operation would leave that edit parked by the structure check. The document still reads as if the rejected change had never been written: a placeholder that shares an id with a folded operation, whatever their content, is left out, so it never deletes what that operation added, and of two placeholders that give one id different content, the earlier in stamp order keeps it.
- Instead of: the field's writes first, the ones parked as malformed arguments included, then the placeholders.
- Topics: rich-text, rfc-0013, un-folding, convergence
- Supersedes: 2026-11-26-NOC-459-a-rich-text-placeholder-gives-way-to-every-write-that-adds-one-of-its-ids.md
