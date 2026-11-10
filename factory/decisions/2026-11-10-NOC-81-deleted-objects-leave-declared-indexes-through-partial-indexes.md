# 2026-11-10 · NOC-81 · Deleted objects leave the declared indexes through partial indexes

- Decided by: coding agent (NOC-81)
- Why: RFC 0006 Amendment 6 leaves the mechanism to the implementation (decision 2). Each `c_<collection>` row gets a column `deleted` (0 or 1), every declared index is a partial index `WHERE deleted = 0`, and every lookup names the same condition, so SQLite walks the partial index and the plan check stays as Amendment 5 wrote it. The fold sets the column when a delete or a restore changes whether the object is deleted, in every collection table that has a row for the object, and a row created after the deletion takes it. The object's fields and its merge state stay, so a read by id still finds it. The new column changes the layout, so the view's format version goes from 1 to 2; a database of format 1 is refused as before.
- Instead of: deleting the row from `c_<collection>` and writing it back on a restore, which would rebuild the row's columns from `fs_<collection>` on every restore; or a separate table of deleted objects joined into every lookup, which the plan check would have to accept as a second table.
- Topics: rfc-0006, view, indexes, sqlite
