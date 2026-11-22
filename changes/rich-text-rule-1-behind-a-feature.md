---
type: internal
tickets: [NOC-457]
components: [core]
---
The core holds rich text, `ncdb/richText` rule 1, behind the cargo feature `rich-text`, which no published build turns on until the fold's guard is complete (NOC-468); without it, rich text parks as before.
