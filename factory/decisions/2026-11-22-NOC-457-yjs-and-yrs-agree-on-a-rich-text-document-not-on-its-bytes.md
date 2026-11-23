# 2026-11-22 · NOC-457 · Yjs and Yrs agree on a rich-text document's items, content and deletions, not on its bytes, and the core keeps Yrs's bytes after a second load

- Decided by: coding agent (NOC-457)
- Why: the agreement vectors showed that Yjs writes a delete set's clients from the largest id down and Yrs from the smallest up, so a state's bytes differ while the documents are equal; and Yrs leaves items that waited as pending cut where they arrived, so its bytes depended on the order of arrival. The core loads each new state once more in one transaction, where Yrs merges items as far as they go, and keeps those bytes; the agreement vectors compare items, content digests and deletions.
- Instead of: comparing bytes with Yjs, which fails for every document with deletions by two clients; keeping Yrs's first encoding, which broke C1.
- Topics: rich-text, rfc-0013, yjs, convergence
