---
type: internal
tickets: [NOC-457]
components: [core]
---
The core's rich text, behind the compiler cfg `ncdb_rich_text`, reads an operation's arguments through `Operation::value()` again, so it builds on `main` after NOC-422 made the field private.
