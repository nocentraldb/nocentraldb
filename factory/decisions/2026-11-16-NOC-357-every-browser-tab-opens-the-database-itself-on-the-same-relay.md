# 2026-11-16 · NOC-357 · Every browser tab opens the database itself on the same relay, and `waitingForTab` means a frozen transaction

- Decided by: Carsten, 2026-11-16, on RFC 0012's questions 2 to 5, through pm (questions 2, 3 and 5)
- Why: with a lock per transaction no tab owns the database, so each tab runs its own worker and core, no tab forwards another's calls and `db.close()` ends only the tab's own core (Q2). One tab syncs at a time and a tab's status names the syncing tab's relay, so a tab whose `nc.open` names another relay than the tabs already open is refused, as a different lock is, and two relays never take turns on one database (Q3). A tab now waits for another only while that tab froze inside a transaction, so `waitingForTab` keeps its name and reports that case (Q5).
- Instead of: keeping the owner's relay for every tab and letting a second relay take turns (Q3); a new name for the narrower wait (Q5). Q2 follows from option B; no other option was put to the owner.
- Topics: browser, tabs, rfc-0012, rfc-0006, rfc-0009, sdk-ts, relay
- Supersedes: 2026-11-13-NOC-177-a-tab-with-another-lock-is-refused-and-the-owners-relay-serves-every-tab.md

The rules stand in `docs/rfc/0012-multi-tab-browser-storage.md`, "What changes for developers", points 2, 3 and 5. The refusal's error name is a public name the implementation ticket brings to the owner. When a write resolves under an outbox (point 6) stays open with the outbox (NOC-364).
