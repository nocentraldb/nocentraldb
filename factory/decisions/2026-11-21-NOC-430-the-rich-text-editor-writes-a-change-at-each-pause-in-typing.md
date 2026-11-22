# 2026-11-21 · NOC-430 · The rich-text editor binding writes one change at each pause in typing: after 500 ms idle, at most every 2 s, and on close or hide

- Decided by: Carsten, 2026-11-21, on RFC 0013's D3, through pm
- Why: collaborators see text within about half a second of a pause, while the relay and the log see writing sessions instead of keystrokes, and a keystroke no longer costs a padded payload, a header and a signature. Up to 2 s of typing can be lost if the tab dies first.
- Instead of: one change per keystroke batch of the editor; a window each app chooses.
- Topics: rich-text, rfc-0013, sdk-ts, relay-metadata
