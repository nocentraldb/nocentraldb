# How long a browser keeps a web app's copy

GENESIS.md promises web apps in Chromium-based browsers, Firefox and Safari, with the data stored in the browser. RFC 0006 (pull request #36) stores a member's copy in SQLite on the Origin Private File System (OPFS), through SQLite's `opfs-sahpool` VFS in a dedicated worker. This page records what each browser actually does with that copy, so that the SDK and its documentation promise that and no more (GENESIS.md, principle 4). It answers NOC-33; how to explain it to developers and their users is NOC-34.

Every row names its source:

- **measured**: we ran it. "measured: Playwright …" rows come from the run committed in [`browser-storage.results.json`](browser-storage.results.json), and a gate test (`scripts/browser-storage.test.mjs`) fails when the table and that run disagree. "measured: manual check <date>" rows come from the [manual check log](#manual-check-log).
- **documented**: the browser vendor's documentation says so, with a link. We have not seen it happen.
- **unknown**: nobody has measured it and no vendor documentation we found says it. The finding then reads "unknown". No row is filled in by guess.

Playwright's builds are not the branded browsers. Its WebKit is not Safari: it has no Intelligent Tracking Prevention, so it never shows the seven-day deletion. Its Chromium is not Chrome or Edge. Rows from Playwright say "(Playwright)"; rows for Safari, Chrome, Edge and Firefox come from documentation or from a manual check.

## Findings

| Browser | Behaviour | Finding | Source |
| --- | --- | --- | --- |
| Chromium (Playwright) | `persist()` | denied, without asking the user | measured: Playwright 1.63.0, Chromium 153.0.8010.12, linux x64, 2026-11-08 |
| Chromium (Playwright) | `persisted()` after `persist()` | false | measured: Playwright 1.63.0, Chromium 153.0.8010.12, linux x64, 2026-11-08 |
| Chromium (Playwright) | `estimate()` | usage 6 MiB, quota 10 GiB | measured: Playwright 1.63.0, Chromium 153.0.8010.12, linux x64, 2026-11-08 |
| Chromium (Playwright) | SQLite on OPFS (`opfs-sahpool`) | works | measured: Playwright 1.63.0, Chromium 153.0.8010.12, linux x64, 2026-11-08 |
| Chromium (Playwright) | Survives a reload | SQLite kept, IndexedDB kept | measured: Playwright 1.63.0, Chromium 153.0.8010.12, linux x64, 2026-11-08 |
| Chromium (Playwright) | Survives a browser restart | SQLite kept, IndexedDB kept | measured: Playwright 1.63.0, Chromium 153.0.8010.12, linux x64, 2026-11-08 |
| Chromium (Playwright) | Survives `Clear-Site-Data: "cache"` | SQLite kept, IndexedDB kept | measured: Playwright 1.63.0, Chromium 153.0.8010.12, linux x64, 2026-11-08 |
| Chromium (Playwright) | Private window: storage (off-the-record context) | SQLite on OPFS works; IndexedDB works | measured: Playwright 1.63.0, Chromium 153.0.8010.12, linux x64, 2026-11-08 |
| Chromium (Playwright) | Private window: after close (off-the-record context) | SQLite lost, IndexedDB lost | measured: Playwright 1.63.0, Chromium 153.0.8010.12, linux x64, 2026-11-08 |
| Firefox (Playwright) | `persist()` | no answer within 5 s: the browser waits for the user | measured: Playwright 1.63.0, Firefox 155.0, linux x64, 2026-11-08 |
| Firefox (Playwright) | `persisted()` after `persist()` | false | measured: Playwright 1.63.0, Firefox 155.0, linux x64, 2026-11-08 |
| Firefox (Playwright) | `estimate()` | usage 576 KiB, quota 10 GiB | measured: Playwright 1.63.0, Firefox 155.0, linux x64, 2026-11-08 |
| Firefox (Playwright) | SQLite on OPFS (`opfs-sahpool`) | works | measured: Playwright 1.63.0, Firefox 155.0, linux x64, 2026-11-08 |
| Firefox (Playwright) | Survives a reload | SQLite kept, IndexedDB kept | measured: Playwright 1.63.0, Firefox 155.0, linux x64, 2026-11-08 |
| Firefox (Playwright) | Survives a browser restart | SQLite kept, IndexedDB kept | measured: Playwright 1.63.0, Firefox 155.0, linux x64, 2026-11-08 |
| Firefox (Playwright) | Survives `Clear-Site-Data: "cache"` | SQLite kept, IndexedDB kept | measured: Playwright 1.63.0, Firefox 155.0, linux x64, 2026-11-08 |
| Firefox (Playwright) | Private window: storage (private browsing mode) | no SQLite on OPFS (SecurityError: Security error when calling GetDirectory); IndexedDB works | measured: Playwright 1.63.0, Firefox 155.0, linux x64, 2026-11-08 |
| Firefox (Playwright) | Private window: after close (private browsing mode) | SQLite: no OPFS, IndexedDB lost | measured: Playwright 1.63.0, Firefox 155.0, linux x64, 2026-11-08 |
| WebKit (Playwright) | `persist()` | denied, without asking the user | measured: Playwright 1.63.0, WebKit 26.6, linux x64, 2026-11-08 |
| WebKit (Playwright) | `persisted()` after `persist()` | false | measured: Playwright 1.63.0, WebKit 26.6, linux x64, 2026-11-08 |
| WebKit (Playwright) | `estimate()` | usage 6 MiB, quota 1000 MiB | measured: Playwright 1.63.0, WebKit 26.6, linux x64, 2026-11-08 |
| WebKit (Playwright) | SQLite on OPFS (`opfs-sahpool`) | works | measured: Playwright 1.63.0, WebKit 26.6, linux x64, 2026-11-08 |
| WebKit (Playwright) | Survives a reload | SQLite kept, IndexedDB kept | measured: Playwright 1.63.0, WebKit 26.6, linux x64, 2026-11-08 |
| WebKit (Playwright) | Survives a browser restart | SQLite kept, IndexedDB kept | measured: Playwright 1.63.0, WebKit 26.6, linux x64, 2026-11-08 |
| WebKit (Playwright) | Survives `Clear-Site-Data: "cache"` | SQLite kept, IndexedDB kept | measured: Playwright 1.63.0, WebKit 26.6, linux x64, 2026-11-08 |
| WebKit (Playwright) | Private window: storage (ephemeral context) | no SQLite on OPFS (UnknownError: The operation failed for an unknown transient reason (e.g. out of memory).); IndexedDB works | measured: Playwright 1.63.0, WebKit 26.6, linux x64, 2026-11-08 |
| WebKit (Playwright) | Private window: after close (ephemeral context) | SQLite: no OPFS, IndexedDB lost | measured: Playwright 1.63.0, WebKit 26.6, linux x64, 2026-11-08 |
| Safari (macOS) | Seven-day deletion of script-writable storage | after seven days of Safari use without user interaction on the site, all of its script-writable storage is deleted; named: IndexedDB, LocalStorage, SessionStorage, media keys, service worker registrations and cache; OPFS is not named | documented: [WebKit blog, 2020-03-24](https://webkit.org/blog/10218/full-third-party-cookie-blocking-and-more/) |
| Safari (macOS) | Seven-day deletion reaches SQLite on OPFS | unknown | unknown |
| Safari (macOS) | `persist()` | granted "based on heuristics like whether the website is opened as a Home Screen Web App"; no prompt is described | documented: [WebKit blog, 2023-08-10](https://webkit.org/blog/14403/updates-to-storage-policy/) |
| Safari (macOS) | `persist()` prevents the seven-day deletion | unknown | unknown |
| Safari (macOS) | Eviction under storage pressure | best-effort by default; an origin "might be excluded from eviction" if it has an active page or its storage is in persistent mode; the policy covers File System storage | documented: [WebKit blog, 2023-08-10](https://webkit.org/blog/14403/updates-to-storage-policy/) |
| Safari (macOS) | Private window | unknown | unknown |
| Safari (iOS) | Seven-day deletion of script-writable storage | as Safari (macOS): the cap applies to "iOS and iPadOS 13.4 and Safari 13.1 on macOS" and later | documented: [WebKit blog, 2020-03-24](https://webkit.org/blog/10218/full-third-party-cookie-blocking-and-more/) |
| Safari (iOS) | Seven-day deletion reaches SQLite on OPFS | unknown | unknown |
| Safari (iOS) | `persist()` prevents the seven-day deletion | unknown | unknown |
| Safari (iOS) | Add to Home Screen prevents the seven-day deletion | web apps on the home screen are not part of Safari and have their own counter of days of use, which their use resets; WebKit does "not expect" their data to be deleted | documented: [WebKit blog, 2020-03-24](https://webkit.org/blog/10218/full-third-party-cookie-blocking-and-more/) |
| Safari (iOS) | Private window | unknown | unknown |
| Chrome, Edge (desktop) | `persist()` | no prompt: granted when the site is "considered important" (engagement, installed or bookmarked, notification permission), otherwise silently denied | documented: [web.dev, persistent storage](https://web.dev/articles/persistent-storage) |
| Chrome, Edge (desktop) | Private window (Incognito, InPrivate) | unknown | unknown |
| Chrome (Android) | Eviction under storage pressure | unknown | unknown |
| Edge (Android) | Eviction under storage pressure | unknown | unknown |
| Firefox | `persist()` | the user is asked in a popup | documented: [MDN, storage quotas and eviction criteria](https://developer.mozilla.org/en-US/docs/Web/API/Storage_API/Storage_quotas_and_eviction_criteria) |
| Firefox | Quota | best-effort: the smaller of 10% of the disk and 10 GiB; persistent: up to 50% of the disk, at most 8 TiB | documented: [MDN, storage quotas and eviction criteria](https://developer.mozilla.org/en-US/docs/Web/API/Storage_API/Storage_quotas_and_eviction_criteria) |
| Firefox | Private window | unknown | unknown |
| All browsers | Eviction under storage pressure | best-effort data of the least recently used origin is deleted first | documented: [MDN, storage quotas and eviction criteria](https://developer.mozilla.org/en-US/docs/Web/API/Storage_API/Storage_quotas_and_eviction_criteria) |
| All browsers | Survives clearing the cache in the browser's settings, keeping site data | unknown | unknown |

Where this differs from RFC 0006 or from what the SDK should promise is for pm to turn into tickets; this page only reports.

## How it was measured

`scripts/browser-storage/` holds the probe:

- `page/` is a static page. Its worker opens `/probe.sqlite3` with SQLite's `opfs-sahpool` VFS (`@sqlite.org/sqlite-wasm`, pinned in `package.json`; RFC 0006 picks the same VFS, in its Rust port), and the page writes the same token to IndexedDB as a control. Its buttons (Write, Read, Request persist(), Storage info) run each step by hand; Write stores the current time, so a read days later shows when the data was written.
- `server.mjs` serves the page on `http://localhost`, which browsers treat as a secure context, so OPFS is available. `/clear-cache` answers with `Clear-Site-Data: "cache"`, which asks the browser to drop its HTTP cache for the origin and keep site data ([MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Clear-Site-Data)).
- `measure.mjs` drives the page with Playwright, headless, one engine at a time, with a fresh profile directory:
  - `persist()` is called first; when it has not answered after 5 s, the row says the browser waits for the user (a headless browser shows no prompt anybody could answer). `persisted()` and `estimate()` are read after it.
  - Each survival row writes a fresh token, runs the event, and reads the token back from SQLite and from IndexedDB: a reload of the page; a visit to `/clear-cache` and back; closing the browser and launching it again on the same profile directory.
  - Private window: Chromium's and WebKit's non-persistent browser contexts (an off-the-record profile and an ephemeral data store), and Firefox with `browser.privatebrowsing.autostart`, its private browsing mode. Playwright's own Firefox contexts are not private (OPFS works in them), and the pref takes effect only from a profile's second launch, so the probe launches the profile once before it measures. "After close" reads in a new private window: a new context of the same browser, or for Firefox a restart on the same profile.
- `run.mjs` runs all three engines, writes the results file and prints the Playwright rows of the table above.

To run it again:

```sh
pnpm exec playwright install --with-deps chromium firefox webkit
node scripts/browser-storage/run.mjs --out docs/browser-storage.results.json
```

Then replace the "(Playwright)" rows above with the printed ones; the gate fails until they match. `node --test 'scripts/browser-storage/*.test.mjs'` (the moon task `repo:browser-storage`) checks that the probe reports every behaviour in every engine. The workflow `browser-storage` runs that test and the probe on GitHub's Ubuntu runner and writes the rows into the run's summary; it does not change this page.

Not covered by the automated run: a branded browser, macOS, Windows, a phone, the seven-day deletion, eviction under storage pressure, a prompt a person answers, and clearing the cache from the browser's settings.

## Manual checks

These need a person with the device. [browser-storage-checklist.md](browser-storage-checklist.md) has the steps per device (Safari on macOS, Safari on iOS in the browser and as a home-screen app, Chrome and Edge on Android, Firefox), what to record, and how long each step takes. Record each check in the log below, then change the row's finding and set its source to `measured: manual check <date>`.

The page for a phone is a static folder: `node scripts/browser-storage/build.mjs <dir>` writes it. The workflow `browser-storage-pages` publishes it to GitHub Pages when the owner has enabled Pages and starts it; until then, every run of the `browser-storage` workflow uploads it as the artifact `browser-storage-probe`, which any static HTTPS host can serve.

## Manual check log

No manual check has been run yet. Add one entry per check, newest last:

```markdown
### YYYY-MM-DD · Safari 26.x on iOS 26.x, iPhone model

- Check: 1, the seven-day deletion
- Method: what you did, with the dates
- Saw: what the page showed
```
