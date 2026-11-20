# 2026-11-07 · NOC-8 · An object's deletion is a boolean basic value, sent as operation 4 delete, and can be undone

- Decided by: Carsten (RFC 0004 decision 4; Amendment 1 decision 14, Carsten, 2026-11-08, on NOC-37)
- Why: with the basic value rule nothing is lost and no new proof is needed: a deletion can be undone, edits to a deleted object are kept and show again on restore, and links to it read **deleted**. It travels as its own operation, number 4 on the object without a field, with set's arguments (`true` deletes, `false` restores, `replaces` names the deletes it saw), so a client that does not know deletion parks it instead of misreading it.
- Instead of: deletion that is final and wins over everything (a late edit would be lost silently); a set on a reserved field such as `$deleted` (every member special-cases a field no lock declares); a set with no field (set's meaning would hang on a missing key).
- Topics: rfc-0004, merge-rule, wire-format

The rules are in `docs/rfc/0004-merge-rules-basic-types.md`, "Creating and deleting an object". Who may delete, what an app sees of a deleted object, and how deletion meets compaction and erasure belong to a later RFC on deletion.
