# 2026-11-09 · NOC-58 · RFC 0006 decision 12 stays A after the private-window reload was measured

- Decided by: owner (Carsten, 2026-11-09, in chat with pm), confirming RFC 0006 Amendment 2, decision 12
- Why: NOC-58 measured that IndexedDB, and with it the member key, survives a reload within one open private window in Playwright's Chromium, Firefox and WebKit, which decision 12's recommendation had called unknown. So a reload in a private window is not a new device. That removes one argument for A but does not change the outcome: where OPFS is missing, the copy stays in memory and is lost with a reload, and the user is told the same either way. RFC 0006 Amendment 4 records the measured facts and this confirmation and decides nothing new.
- Instead of: reopening decision 12 for B, a copy in IndexedDB that would outlive a reload, which adds a second storage path to build and test in three engines.
- Topics: rfc-0006, browser-storage, private-window
