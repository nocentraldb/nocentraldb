# 2026-11-27 · NOC-579 · NcMention takes a label that resolves the target's space and id, and no db option

- Decided by: Carsten, 2026-11-27, on NOC-579
- Why: the mention node holds only `space` and `id` (RFC 0013, "Mentions"), and reading an object needs its collection, so the app, which knows its collections, resolves the name: `NcMention.configure({ label: (target: { space, id }) => string | undefined | Promise<string | undefined> })`. It needs no new SDK call and works for mentions of any collection.
- Instead of: a `db` option with one fixed collection, or a mention node that also stores the collection, which changes the node RFC 0013 defines.
- Topics: rich-text, rfc-0013, tiptap, mentions, personal-data
