# 2026-11-21 · NOC-430 · Rich text is not personal in rule 1, and a mention of a person stores only a reference to the person object, as a recommendation the core does not enforce

- Decided by: Carsten, 2026-11-21, on RFC 0013's D7, through pm
- Why: the person object's personal fields can be protected and erased, so a name that is never written into the text is never left behind by an erasure. The core cannot tell a name from other text, so this is a recommendation in RFC 0013 and the docs, not a rule.
- Instead of: a personal rich-text field whose every update is sealed with the person key.
- Topics: rich-text, rfc-0013, erasure, personal-fields
