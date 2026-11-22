# 2026-11-22 · NOC-458 · A device opens a lock with rich text and parks its operations as an unknown type until the core folds it

- Decided by: coding agent (NOC-458)
- Why: step 2b puts `ncdb/richText` into the lock before step 2a gives the core its rule; parking keeps every operation, as for a plugin type without code (RFC 0011), and the parked operations fold once the core has the rule, so nothing written in between is lost or folded by a wrong rule.
- Instead of: refusing to open a database whose lock has a rich-text field, which would stop an app that only declares the field; or folding the field as a basic value, which would fold Yjs updates by the wrong rule.
- Topics: rich-text, model-lock, field-types

Step 2a replaces this: it gives the view a field type for `ncdb/richText` and removes rich text from the view's unregistered fields (`crates/ncdb-core/src/view/from_lock.rs`).
