# 2026-11-28 · NOC-536 · A first document from Markdown refuses every `ncdb:` link that is not a mention's link form, while an anchored edit's replacement still reads one as a plain link

- Decided by: coding agent (NOC-536)
- Why: the ticket refuses a malformed mention instead of storing it as a plain link. The core cannot tell a malformed mention from any other `ncdb:` link, so a first document refuses every `ncdb:` link other than `[@](ncdb:<space id>/<object id>)` with two ids of 32 lowercase hexadecimal digits: one without an id, with ids in another form, with a part after the object id, with its scheme in capitals or with a name as its text. Anchored edits share the parser but are outside this ticket, so they keep reading such a link as a link until pm decides otherwise.
- Instead of: writing such a link as a plain link, which hides a broken mention and can carry a name into the text; refusing it in anchored edits too, which changes behaviour no ticket asked for.
- Topics: rich-text, rfc-0013, mentions, sdk-rust, sdk-ts
