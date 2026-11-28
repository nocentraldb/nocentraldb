# 2026-11-28 · NOC-587 · ncEditor builds the TipTap editor on a rich-text binding and hands each one to onEditor; NcKeepUnknown takes document and extensions; an editor declares its placeholders as KnownNames

- Decided by: Carsten, 2026-11-28, on NOC-587
- Why: the names say what each part does in the words RFC 0013 uses: `ncEditor(body, { element, extensions, editorOptions, onEditor })` builds and rebuilds the editor on `body` and hands each one to `onEditor`; `NcKeepUnknown.configure({ document, extensions })` widens an editor an app builds itself from the field's document and the app's extensions; `KnownNames` (`nodes`, `marks`, `attributes`) is what an editor declares to `body.known`.
- Instead of: other names for the call, its options or the type, chosen before the package is published.
- Topics: rich-text, rfc-0013, tiptap, sdk-ts, naming
