# 2026-11-16 · NOC-357 · The owner decides RFC 0012, with every tab opening the database itself and a tab with another relay refused

- Decided by: Carsten, 2026-11-16, on RFC 0012's questions 2 to 5, through pm
- Why: with option B chosen (2026-11-16-NOC-357-browser-tabs-take-the-database-per-transaction.md), the owner answered the rest. Every tab opens the database itself, so RFC 0006's owning tab and RFC 0009's "Open and close" are rewritten (Q2). A tab opened with another relay than the tabs already open is refused, so two relays never take turns on one database (Q3). The higher cost of a write, 8 ms instead of 1.5 ms in WebKit in the spike, is accepted and measured in practice: "we'll see, it is worth the risk" (Q4). `waitingForTab` keeps its name and now means that another tab froze inside a transaction (Q5). With these answers RFC 0012 is decided; its follow-up tickets rewrite RFC 0006 and RFC 0009 and build the design.
- Instead of: keeping the owner's relay for every tab and letting a second relay take turns (Q3); holding B back until a faster resume (Q4); a new name for the narrower wait (Q5). When a write resolves under an outbox (Q6) stays open with the outbox, deferred until telemetry (NOC-364).
- Topics: browser, tabs, rfc-0012, rfc-0006, rfc-0009, sdk-ts
- Supersedes: 2026-11-13-NOC-177-a-tab-with-another-lock-is-refused-and-the-owners-relay-serves-every-tab.md
