# 2026-11-28 · NOC-590 · A write of nc.richText.fromMarkdown on a core without rich text answers notSupported, as space.richText does

- Decided by: Carsten, 2026-11-28, on NOC-590 (2a), through pm
- Why: an app asks one question, whether this build holds rich text, and gets the same answer from both calls; a rich-text field keeps taking only `nc.richText.fromMarkdown(...)` in a write (1a), so a first document's content check stays in one place.
- Instead of: refusing the write with `invalidValue`, as the core does for a value it cannot write; or also accepting a plain string as Markdown.
- Topics: rich-text, rfc-0013, sdk-ts, errors
