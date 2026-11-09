# 2026-11-08 · NOC-33 · "Cleared cache" is measured with `Clear-Site-Data: "cache"`; the settings route stays a manual check

- Decided by: coding agent (NOC-33)
- Why: Playwright cannot press "clear cached files" in a browser's settings in all three engines. The header is the one cache clear that works the same way in every engine and keeps site data; the row names it. Clearing the cache from the settings is a row of its own, "unknown" until a person checks it.
- Instead of: Chromium's DevTools protocol only, which would leave Firefox and WebKit without the row.
- Topics: browser-storage, testing
