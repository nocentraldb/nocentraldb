---
type: internal
tickets: [NOC-584]
components: [core]
---
The core's rule for a rich-text item id written twice, behind the compiler cfg `ncdb_rich_text`, compares content with every map in key order, so a writer that sends the same items again with a map in another order no longer parks.
