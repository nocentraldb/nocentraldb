# 2026-11-22 · NOC-467 · The rich-text binding holds a write during an IME composition, until compositionend and for at most 2 s

- Decided by: Carsten, 2026-11-22, on NOC-467, through pm
- Why: the browser spike (NOC-455) showed the pause writing characters the person had not committed yet; collaborators and the log would see them. The 2 s cap keeps a person who types through an IME no further behind than one who types without a pause.
- Instead of: writing the uncommitted text at the pause, as for any other edit.
- Topics: rich-text, rfc-0013, sdk-ts, ime
