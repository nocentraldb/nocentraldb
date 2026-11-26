---
type: internal
tickets: [NOC-539]
components: [sdk-rust]
---
With the cfg `ncdb_rich_text`, a move or an ad-hoc copy of an object whose rich-text field was written is refused with `notAllowed`, naming the field, instead of dropping the field.
