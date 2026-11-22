# 2026-11-21 · NOC-436 · An agent edits rich text by quoting Markdown, looked up in the version it read and written as a Yjs update made at that version

- Decided by: Carsten, 2026-11-21, on RFC 0013's D5, through pm
- Why: the spike on NOC-436 (RFC 0013's F11 to F14; code on the branch carstenkoch/noc-436-research-how-an-agent-and-a-person-edit) found that a quote looked up at write time fails exactly when a person types in the same sentence, while a quote looked up in the read version applies and Yjs keeps the person's typing, even inside the quote. Markdown stays the quote's language, as it is what the agent reads. The edit goes under the agent's leased client id, its edits for one field in order. It needs D4's A or B; with D4's C it works on plain-text quotes only.
- Instead of: looking the quote up in the document at write time (the draft); plain text plus block calls; node ids with whole-node content; an editing session with a cursor, which can come later on top of this, with presence.
- Topics: rich-text, rfc-0013, agents, yjs
