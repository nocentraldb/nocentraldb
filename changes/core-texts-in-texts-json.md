---
type: internal
tickets: [NOC-342]
components: [core]
---
The rest of the core's texts move into `texts/texts.json` as texts of kind `core`, with when each one shows: the errors of reading a lock or a schema's description, of a change, a seal, a key envelope, a control body, the clock, plugins, a recovery code, a pairing answer and the local view, and the values that name what the view could not read. No wording changes. The gate `repo:texts` now reads all of `crates/ncdb-core/src`: there a word has a lower-case letter, so SQL is no text, a panic's message is no text, and an inline test module is a test.
