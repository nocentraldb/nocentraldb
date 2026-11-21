# 2026-11-21 · NOC-430 · `space.get` returns a rich-text field as its Markdown, a string

- Decided by: Carsten, 2026-11-21, on RFC 0013's D6, through pm
- Why: a list shows a preview without another call, `nc.Infer` types the field as `string`, and an editor edits through the binding. Which vocabulary the Markdown knows stays open with D4, which waits for D5 (NOC-436).
- Instead of: an object with `markdown()`, `text()` and `tree()`, which would make a read object no longer plain data.
- Topics: rich-text, rfc-0013, sdk-ts, reads
