# 2026-11-23 · NOC-454 · A decided RFC names nobody who decided: its status reads "Decided, YYYY-MM-DD." and a Decisions entry keeps the date, what changed and its decision link

- Decided by: Carsten, 2026-11-23, on NOC-454 (B), through pm
- Why: readers of an RFC do not care who decided, and the decision log already records it in `Decided by:`. A later decision is written into the RFC's text; its Decisions entry keeps its date, what changed and the link to its entry in the log, or the ticket where it has none. The status line reads "Decided, YYYY-MM-DD.", followed by any other status the RFC gives, such as what is built. `scripts/lib/rfc-record.mjs` refuses an entry that names a decider, and `scripts/rfc-status.test.mjs` refuses a status line that does. Every decided RFC follows the form, so decided RFCs and new ones read in one style. Names inside an RFC's running text stay until a ticket of their own removes them.
- Instead of: keeping the rule as it was, with decided RFCs naming who decided under Decisions and in their status while new RFCs read without names (A).
- Topics: rfc, decision-log, gate
- Supersedes: 2026-11-20-NOC-378-an-rfc-is-decided-when-its-status-line-says-so.md
