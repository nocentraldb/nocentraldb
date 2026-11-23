# 2026-11-22 · NOC-457 · A rich-text field folds again in stamp order, parked writes of its ids included, when a write displaces a later write of its ids

- Decided by: coding agent (NOC-457)
- Why: when the earlier of two writes of one id arrives after the later was folded, the later must come out of the document, and an operation parked because of the later one may now fit. Folding every operation on the field again from the earliest in stamp order, the ones parked as malformed arguments included, gives each the verdict it has in every order of arrival, and the log records what parks and what no longer does.
- Instead of: parking only the displaced write and leaving the document, which keeps its content under the earlier write's ids; refolding in log order, which would fold the later write first again.
- Topics: rich-text, rfc-0013, convergence, un-folding

Un-folding a rejected change (step 3a) does not yet look at writes that parked because of the rejected one; 3a takes that up.
