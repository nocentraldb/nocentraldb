---
type: internal
tickets: [NOC-460]
components: [core]
---
The core reads a rich-text field, behind the compiler cfg `ncdb_rich_text`, as Markdown in TipTap's vocabulary with an HTML tag for any other part, as plain text, as a ProseMirror tree read from Yrs, and as the references its mentions hold (RFC 0013, step 3b).
