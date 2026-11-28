# 2026-11-28 · NOC-612 · Markdown input reads `[@](ncdb:<space id>/<object id>)`, its scheme in any case, as a mention and refuses every other `ncdb:` link with `invalidValue`, in a first document and in anchored edits alike

- Decided by: Carsten, 2026-11-28, on NOC-612, through pm
- Why: a malformed or named `ncdb:` link written as a plain link would hide a broken mention from the app that imported it, and a name in the link text puts a person's name into the text, which RFC 0005's "Personal data in free text" pattern avoids; the app sees the refusal and fixes its export. URI schemes are case-insensitive (RFC 3986), so `NCDB:` or `Ncdb:` with two valid ids means the same mention, and it reads back as `ncdb:`. Both paths that read Markdown give the same result: `fromMarkdown` and `RichText::from_markdown` (NOC-536) and an anchored edit's replacement (NOC-574).
- Instead of: a named `ncdb:` link written as a plain link, or read as a mention with the name dropped; refusing the scheme in capitals; leaving anchored edits to write a malformed `ncdb:` link as a plain link.
- Topics: rich-text, rfc-0013, mentions, sdk-rust, sdk-ts
