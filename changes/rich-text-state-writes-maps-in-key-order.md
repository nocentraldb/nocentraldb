---
type: internal
tickets: [NOC-540]
components: [core]
---
The core writes every map of a rich-text field's encoded document in key order, behind the compiler cfg `ncdb_rich_text`, so copies that fold the same updates hold the same state bytes also for marks with several attributes, and `Read::xml` is gone in favour of the step 3b reads.
