---
type: feature
tickets: [NOC-156]
components: [core]
---
## Problem
Opening a database with an additive change of the model, such as a new field, a new collection or a new index, folded the member's whole log again before `open` returned. On a large database that blocked the app at start.

## Use
Nothing to call. When the lock only adds to the model the view was folded by, the view is migrated in place: a new index is created, a new field gets its column, a new collection gets its tables, and only the changes with operations parked for them are opened and folded again. Any other change of the model still folds the whole log again, as before.
