# 2026-11-09 · NOC-34 · One page explains browser durability with texts for users, and the SDK warns only in development

- Decided by: Carsten, 2026-11-09, in chat with pm, on #73 (RFC 0006 decisions 13, 14 and 15)
- Why: a developer has to know the limits before choosing a browser app for their users, which is before reading an API reference, so one page, "Browser apps and durability", states NOC-33's findings per browser (13). The app owns its interface and languages, so the page carries short English texts with `{appName}` and `{count}` and the SDK ships none (14). The README and RFC 0001 link the page, and in development the SDK logs one `console.warn` per page load when `storage` is not `persistent` (off with `warnings: false`), so a developer sees the limit on the first day while the end user's console stays quiet (15).
- Instead of: the facts only in the SDK's API reference (13); texts exported by the SDK, such as `nc.text.durability.unsaved({ appName, count })` (14); the warning in every build, or no warning (15).
- Topics: rfc-0006, browser, durability, docs, user-text

`docs/rfc/0006-local-view.md`, "What developers and their users read", states the rules.
