# 2026-11-09 · NOC-77 · A plugin field has no field type in the view yet, so its writes park

- Decided by: coding agent (NOC-77), for pm's review
- Why: the core has no plugin field types yet. `view::Model::from_lock` leaves a plugin's field out of the view's model, so the fold parks its operations as it parks any field its model does not know, and keeps them (GENESIS principle 3: a client keeps data it does not understand yet). The field gets no column and no merge state until the plugin ticket gives it a field type; an index on it is refused by the lock's own checks already.
- Instead of: refusing to open a view on a lock with a plugin field, which would stop every member of a model that uses one.
- Topics: view, model-lock, plugins
