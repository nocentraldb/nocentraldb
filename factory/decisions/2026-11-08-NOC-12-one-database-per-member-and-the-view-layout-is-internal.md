# 2026-11-08 · NOC-12 · One database per member holds all its spaces, and the view's layout is internal, not a stable format

- Decided by: Carsten (RFC 0006 decisions 1 and 2)
- Why: with a space id in every row, a declared index spans spaces, so "my next 50 tasks across every project" is one range lookup, and leaving a space deletes its rows; the view's tables are internal to the core, versioned in `meta`, and a new core may rebuild them, since the view can always be rebuilt from the log and apps never see the tables.
- Instead of: one database per space; a view layout versioned and migrated like the log.
- Topics: rfc-0006, local-view, sqlite, on-disk-format

`docs/rfc/0006-local-view.md`, "Where the view lives", states the layout.
