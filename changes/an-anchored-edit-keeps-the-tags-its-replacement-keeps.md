---
type: internal
tickets: [NOC-574]
components: [core]
---
An anchored edit, behind the compiler cfg `ncdb_rich_text`, now edits the text around a tag in the blocks its quote touches when the replacement keeps the tag byte for byte, and leaves the tag's node or mark untouched (RFC 0013, "Parts outside the vocabulary").
