# 2026-11-27 · NOC-556 · The rich-text binding defers its pull of folded changes while an IME composition is open, until compositionend and for at most 2 s

- Decided by: Carsten, 2026-11-27, on NOC-556, through pm
- Why: the spike on NOC-555 showed TipTap ending a composition itself when it meets a node its schema lacks, before onContentError runs, so a rebuild that waits for compositionend cannot protect the composition; deferring the pull keeps the new node away from the editor until the composition ends, which then finished intact.
- Instead of: a rebuild that waits for compositionend, which TipTap's own handling of the unknown node makes moot.
- Topics: rich-text, rfc-0013, sdk-ts, ime, unknown-nodes
- Supersedes: 2026-11-22-NOC-467-a-rich-text-editor-rebuild-waits-for-compositionend-and-restores-focus-and-selection.md
