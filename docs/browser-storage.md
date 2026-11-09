# How long a browser keeps a web app's copy

GENESIS.md promises web apps in Chromium-based browsers, Firefox and Safari, with the data stored in the browser. [RFC 0006](rfc/0006-local-view.md) stores a member's copy in SQLite on the Origin Private File System (OPFS), through SQLite's `opfs-sahpool` VFS in a dedicated worker. This page records what each browser actually does with that copy, so that the SDK and its documentation promise that and no more (GENESIS.md, principle 4). It answers NOC-33; how to explain it to developers and their users is NOC-34.

Every row names its source:

- **measured**: we ran it. "measured: Playwright …" rows come from the run committed in [`browser-storage.results.json`](browser-storage.results.json), and a gate test (`scripts/browser-storage.test.mjs`) fails when the table and that run disagree. "measured: manual check <entry>" rows come from the entry of that heading in the [manual check log](#manual-check-log), and the gate test fails when the log has no such entry.
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
| Safari (macOS) | SQLite on OPFS (`opfs-sahpool`) | works: the time written was read back from SQLite and from IndexedDB | measured: manual check 2026-11-09 · Safari 27.2 on macOS 26.7.2, Apple M1 Pro (2021) |
| Safari (macOS) | `persist()` | granted "based on heuristics like whether the website is opened as a Home Screen Web App"; no prompt is described | documented: [WebKit blog, 2023-08-10](https://webkit.org/blog/14403/updates-to-storage-policy/) |
| Safari (macOS) | `persist()` in a Dock app | granted; `persisted()` true afterwards; the log records no prompt | measured: manual check 2026-11-09 · Dock app (Safari 27.2) on macOS 26.7.2, Apple M1 Pro (2021) |
| Safari (macOS) | `persist()` prevents the seven-day deletion | unknown | unknown |
| Safari (macOS) | Eviction under storage pressure | best-effort by default; an origin "might be excluded from eviction" if it has an active page or its storage is in persistent mode; the policy covers File System storage | documented: [WebKit blog, 2023-08-10](https://webkit.org/blog/14403/updates-to-storage-policy/) |
| Safari (macOS) | Private window | unknown | unknown |
| Safari (iOS) | Seven-day deletion of script-writable storage | as Safari (macOS): the cap applies to "iOS and iPadOS 13.4 and Safari 13.1 on macOS" and later | documented: [WebKit blog, 2020-03-24](https://webkit.org/blog/10218/full-third-party-cookie-blocking-and-more/) |
| Safari (iOS) | Seven-day deletion reaches SQLite on OPFS | unknown | unknown |
| Safari (iOS) | SQLite on OPFS (`opfs-sahpool`) | works: the time written was read back from SQLite and from IndexedDB | measured: manual check 2026-11-09 · Safari 27.0.1 on iOS 27.0.1, iPhone 15 |
| Safari (iOS) | `persist()` | denied; the log records no prompt | measured: manual check 2026-11-09 · Safari 27.0.1 on iOS 27.0.1, iPhone 15 |
| Safari (iOS) | `persist()` in a home-screen app | denied; `persisted()` false afterwards; the log records no prompt | measured: manual check 2026-11-09 · Home-screen app (Safari 27.0.1) on iOS 27.0.1, iPhone 15 |
| Safari (iOS) | `persist()` prevents the seven-day deletion | unknown | unknown |
| Safari (iOS) | Add to Home Screen prevents the seven-day deletion | web apps on the home screen are not part of Safari and have their own counter of days of use, which their use resets; WebKit does "not expect" their data to be deleted | documented: [WebKit blog, 2020-03-24](https://webkit.org/blog/10218/full-third-party-cookie-blocking-and-more/) |
| Safari (iOS) | Private window | unknown | unknown |
| Chrome, Edge (desktop) | `persist()` | no prompt: granted when the site is "considered important" (engagement, installed or bookmarked, notification permission), otherwise silently denied | documented: [web.dev, persistent storage](https://web.dev/articles/persistent-storage) |
| Chrome, Edge (desktop) | Private window (Incognito, InPrivate) | unknown | unknown |
| Chrome (Android) | Eviction under storage pressure | unknown | unknown |
| Edge (Android) | Eviction under storage pressure | unknown | unknown |
| Firefox | `persist()` | the user is asked in a popup | documented: [MDN, storage quotas and eviction criteria](https://developer.mozilla.org/en-US/docs/Web/API/Storage_API/Storage_quotas_and_eviction_criteria) |
| Firefox (macOS) | `persist()` | a prompt asks "Allow nocentraldb.github.io to store data in persistent storage?"; Allow grants it and `persisted()` turns true. Asked again after the permission was cleared, meant to be blocked, it also answered granted; the log does not say whether the prompt appeared again | measured: manual check 2026-11-09 · Firefox 156.0.1 on macOS 26.7.2, Apple M1 Pro (2021) |
| Firefox | Quota | best-effort: the smaller of 10% of the disk and 10 GiB; persistent: up to 50% of the disk, at most 8 TiB | documented: [MDN, storage quotas and eviction criteria](https://developer.mozilla.org/en-US/docs/Web/API/Storage_API/Storage_quotas_and_eviction_criteria) |
| Firefox | Private window | no SQLite on OPFS (SecurityError: Security error when calling GetDirectory); IndexedDB writes, but a Read in a new private window a minute later showed `null`; the log does not say whether the first private window was closed first | measured: manual check 2026-11-09 · Firefox 156.0.1, private window, on macOS 26.7.2, Apple M1 Pro (2021) |
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

These need a person with the device. [browser-storage-checklist.md](browser-storage-checklist.md) has the steps per device (Safari on macOS, Safari on iOS in the browser and as a home-screen app, Chrome and Edge on Android, Firefox), what to record, and how long each step takes. Record each check in the log below, then change the row's finding and set its source to `measured: manual check <entry heading>`, for example `measured: manual check 2026-11-09 · Safari 27.2 on macOS 26.7.2, Apple M1 Pro (2021)`. A behaviour a vendor documents keeps its documented row; the measurement gets a row of its own below it.

The page for a phone is a static folder: `node scripts/browser-storage/build.mjs <dir>` writes it. The workflow `browser-storage-pages` publishes it to GitHub Pages when the owner has enabled Pages and starts it; until then, every run of the `browser-storage` workflow uploads it as the artifact `browser-storage-probe`, which any static HTTPS host can serve.

## Manual check log

Add one entry per check, newest last, with the page's output shortened to the fields that matter:

```markdown
### YYYY-MM-DD · Safari 26.x on iOS 26.x, iPhone model

- Check: the checklist's section and step, e.g. Safari on iOS, in the browser, step 1 (day 0)
- Method: what you did, with the dates
- Saw: what the page showed
```

### 2026-11-09 · Safari 27.0.1 on iOS 27.0.1, iPhone 15

- Check: Safari on iOS, in the browser, step 1 (day 0).
- Method: on 2026-11-09, opened the probe page in Safari and pressed Storage info (13:41 UTC), Write (13:42) and Read (13:42); after the home-screen check below, Request persist() (14:04).
- Saw: SQLite on OPFS and IndexedDB both wrote the time and read it back. `persisted()` false; `estimate()` usage 156 KiB, quota 38.4 GiB. `persist()` answered: denied. The log does not record whether a prompt appeared. The user agent reports "iPhone OS 18_7"; the iOS version is the one the owner noted.

```text
Storage info (2026-11-09T13:41:09.148Z):
{ "userAgent": "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/27.0.1 Mobile/15E148 Safari/604.1",
  "persisted": false, "estimate": { "usage": 159748, "quota": 41231686042 }, "opfs": "ok" }
Write (2026-11-09T13:42:05.842Z): { "sqlite": "ok", "idb": "ok" }
Read (2026-11-09T13:42:22.646Z): { "sqlite": "written 2026-11-09T13:42:05.816Z", "idb": "written 2026-11-09T13:42:05.816Z" }
persist() (2026-11-09T14:04:41.432Z): { "answered": true, "granted": false }
```

### 2026-11-09 · Home-screen app (Safari 27.0.1) on iOS 27.0.1, iPhone 15

- Check: Safari on iOS, as a home-screen app, step 1 (day 0).
- Method: on 2026-11-09, added the probe page to the home screen, opened the app from there and pressed Request persist() (13:44 UTC), Write (13:45), Storage info (13:45) and Read (13:46).
- Saw: `persist()` answered: denied, and `persisted()` stayed false. The log does not record whether a prompt appeared. SQLite on OPFS and IndexedDB both wrote the time and read it back. `estimate()` usage 6 MiB, quota 38.4 GiB.

```text
persist() (2026-11-09T13:44:56.732Z): { "answered": true, "granted": false }
Write (2026-11-09T13:45:30.046Z): { "sqlite": "ok", "idb": "ok" }
Storage info (2026-11-09T13:45:45.751Z):
{ "userAgent": "Mozilla/5.0 (iPhone; CPU iPhone OS 18_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/27.0.1 Mobile/15E148 Safari/604.1",
  "persisted": false, "estimate": { "usage": 6291535, "quota": 41231686042 }, "opfs": "ok" }
Read (2026-11-09T13:46:02.180Z): { "sqlite": "written 2026-11-09T13:45:30.029Z", "idb": "written 2026-11-09T13:45:30.029Z" }
```

### 2026-11-09 · Safari 27.2 on macOS 26.7.2, Apple M1 Pro (2021)

- Check: Safari on macOS, step 1 (day 0).
- Method: on 2026-11-09, opened the probe page in Safari and pressed Storage info (13:57 UTC), Write (13:59) and Read (14:00). Request persist() was not pressed, as the checklist asks.
- Saw: SQLite on OPFS and IndexedDB both wrote the time and read it back. `persisted()` false; `estimate()` usage 6 MiB, quota 76.8 GiB.

```text
Storage info (2026-11-09T13:57:49.702Z):
{ "userAgent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/27.2 Safari/605.1.15",
  "persisted": false, "estimate": { "usage": 6291473, "quota": 82463372084 }, "opfs": "ok" }
Write (2026-11-09T13:59:57.389Z): { "sqlite": "ok", "idb": "ok" }
Read (2026-11-09T14:00:14.178Z): { "sqlite": "written 2026-11-09T13:59:57.362Z", "idb": "written 2026-11-09T13:59:57.362Z" }
```

### 2026-11-09 · Dock app (Safari 27.2) on macOS 26.7.2, Apple M1 Pro (2021)

- Check: Safari on macOS, step 2 (day 0).
- Method: on 2026-11-09, added the probe page to the Dock, opened the app from there and read "Read on load" (14:00 UTC); then pressed Request persist() (14:01), Write (14:02), Storage info (14:02) and Read (14:03).
- Saw: "Read on load" showed `null` for both: the Dock app does not see what Safari wrote a minute earlier. `persist()` answered: granted, and `persisted()` turned true. The log does not record whether a prompt appeared. SQLite on OPFS and IndexedDB both wrote the time and read it back. `estimate()` usage 6 MiB, quota 76.8 GiB.

```text
Read on load (2026-11-09T14:00:57.870Z): { "sqlite": null, "idb": null }
persist() (2026-11-09T14:01:56.519Z): { "answered": true, "granted": true }
Write (2026-11-09T14:02:19.379Z): { "sqlite": "ok", "idb": "ok" }
Storage info (2026-11-09T14:02:25.865Z):
{ "userAgent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/27.2 Safari/605.1.15",
  "persisted": true, "estimate": { "usage": 6291535, "quota": 82463372084 }, "opfs": "ok" }
Read (2026-11-09T14:03:41.107Z): { "sqlite": "written 2026-11-09T14:02:19.357Z", "idb": "written 2026-11-09T14:02:19.357Z" }
```

### 2026-11-09 · Firefox 156.0.1 on macOS 26.7.2, Apple M1 Pro (2021)

- Check: Firefox, steps 1 and 2.
- Method: on 2026-11-09, opened the probe page; "Read on load" (14:08 UTC). Pressed Request persist(): a prompt asked "Allow nocentraldb.github.io to store data in persistent storage?"; the owner set "Remember this decision" and pressed Allow (14:10). Pressed Storage info (14:12). Then, in the owner's words, "Doing it a 2nd time after I deleted the permission (this time blocking)": Request persist() (14:17) and Storage info (14:17).
- Saw: after Allow, `persist()` answered granted and `persisted()` was true; `estimate()` usage 576 KiB, quota 230.2 GiB. The second request, meant to be blocked, also answered granted, and `persisted()` was still true. The log does not say whether the prompt appeared again, or whether the page was reloaded after the permission was cleared. No Write or Read in this check.

```text
Read on load (2026-11-09T14:08:48.199Z): { "sqlite": null, "idb": null }
persist() (2026-11-09T14:10:35.224Z): { "answered": true, "granted": true }
Storage info (2026-11-09T14:12:02.401Z):
{ "userAgent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:156.0) Gecko/20100101 Firefox/156.0",
  "persisted": true, "estimate": { "usage": 589824, "quota": 247192397824 }, "opfs": "ok" }
persist() (2026-11-09T14:17:08.681Z): { "answered": true, "granted": true }
Storage info (2026-11-09T14:17:35.023Z): { "persisted": true, "estimate": { "usage": 589824, "quota": 247192397824 }, "opfs": "ok" }
```

### 2026-11-09 · Firefox 156.0.1, private window, on macOS 26.7.2, Apple M1 Pro (2021)

- Check: Firefox, step 3 (private window).
- Method: on 2026-11-09, opened the probe page in a private window and pressed Storage info (14:18 UTC) and Write (14:19). Then, in the owner's words, "Opening new private window", and pressed Read there (14:20).
- Saw: no OPFS: opening it failed with "SecurityError: Security error when calling GetDirectory", so SQLite did not open. IndexedDB wrote the time. Read in the new private window showed `null` for IndexedDB. `persisted()` false; `estimate()` usage 88 KiB, quota 10 GiB. The log does not say whether the first private window was closed before the new one was opened, as the checklist's step asks.

```text
Storage info (2026-11-09T14:18:55.592Z):
{ "persisted": false, "estimate": { "usage": 90112, "quota": 10737418240 },
  "opfs": "error: SecurityError: Security error when calling GetDirectory" }
Write (2026-11-09T14:19:26.667Z): { "sqlite": "error: SecurityError: Security error when calling GetDirectory", "idb": "ok" }
Read (2026-11-09T14:20:00.037Z): { "sqlite": "error: SecurityError: Security error when calling GetDirectory", "idb": null }
```
