# 2026-11-27 · NOC-461 · A reader's rich-text binding reports readOnly from the start, also when its editor does not fully show the document

- Decided by: pm, NOC-461, following NOC-538 Q2
- Why: the owner decided Q2 on the reading that readOnly covers a member with read rights only as well as a binding whose writes stopped, so an app reads one field for both; readOnly says the most an app must act on.
- Instead of: shown for a reader, with writable the only sign; or notFullyShown when a reader's document also holds parts the editor does not show.
- Topics: rich-text, rfc-0013, sdk-ts
