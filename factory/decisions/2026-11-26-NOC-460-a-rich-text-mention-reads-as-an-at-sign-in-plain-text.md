# 2026-11-26 · NOC-460 · A rich-text mention reads as `@` in plain text

- Decided by: pm (NOC-460)
- Why: a mention holds no name, so plain text has none to show; `@`, the link text of its Markdown form, shows a preview that someone is mentioned, and an app that wants names resolves them from `mentions()`.
- Instead of: nothing, which leaves a preview such as "Ask  about it" with no sign of the mention.
- Topics: rich-text, rfc-0013, reads, mentions
