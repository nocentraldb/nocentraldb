# 2026-11-19 · NOC-359 · The SDK's texts follow the glossary; a text an RFC quotes waits for NOC-362

- Decided by: coding agent, NOC-359, within pm's scope (texts.json only, no public identifier, the glossary is the owner's)
- Why: 26 texts change to use the glossary's terms and one wording per concept, and `scripts/texts-glossary.test.mjs` keeps them so. Texts that RFC 0005 or RFC 0009 quote (the notices, the lock's refusals such as "Clients would merge") stay as they are, since the RFCs are NOC-362's, in flight; their proposed rewordings are listed in the PR. "Client" stays where it means a program on an older model, which the glossary allows ("a client is any program that talks to the relay"). New glossary rows are proposals for the owner; no existing row is reworded.
- Instead of: changing the quoted texts and the RFCs here, which would edit NOC-362's files; or a gate that parses the glossary's Avoid column, whose entries carry exceptions in prose.
- Topics: texts, glossary
