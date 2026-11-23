# 2026-11-23 · NOC-475 · A view records the object operations it knows, and the first open by a newer core folds what an older one parked

- Decided by: coding agent, NOC-475
- Why: RFC 0016 says an old core shows a moved object as live "until it upgrades"; the upgrade must fold the move it parked as an unknown operation and `$movedFrom` as an unknown field. The view's `meta` key `operations` (now `6`) marks that this was done, so later opens open no change for it, as a migration folds only what a new field makes known.
- Instead of: a full refold of every view on upgrade, which opens every change of the log once.
- Topics: rfc-0016, move, view, upgrade, parking
