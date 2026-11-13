---
type: internal
tickets: [NOC-304]
components: [sdk-rust]
---
The test of words that are no recovery code compares each refusal with its exact text, so a code with a word such as "cover" or "over" no longer fails it.
