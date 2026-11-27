---
type: internal
tickets: [NOC-459]
components: [core]
---
The core's rich text, behind the compiler cfg `ncdb_rich_text`, un-folds a rejected change to deleted placeholders without its delete set (RFC 0013), and checks again the writes that parked because of it.
