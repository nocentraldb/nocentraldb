# 2026-11-28 · NOC-611, NOC-605 · A rich-text field folds in stamp order with nothing pending, and a structure check parks what would wait

- Decided by: Carsten, 2026-11-28, on NOC-611, through pm
- Why: determinism; folded in stamp order with nothing waiting as pending, a field's state is a function of the set of its operations, byte for byte, on every member and in the editor.
- Instead of: folding in any order of arrival with content waiting as pending; or leaving the fold as it is until a later library release, or running it in an isolated process.
- What this changes: the core checks each update's structure against the document of the operations before it in stamp order and parks, as "malformed arguments", one that would leave content pending or that no honest writer produces; an honest update that arrives before a change it depends on folds at its place once that change arrives. A change that arrives earlier in stamp order than an operation of the field the member has folded or parked makes the view fold that field again in stamp order.
- Topics: rich-text, rfc-0013, convergence, robustness
