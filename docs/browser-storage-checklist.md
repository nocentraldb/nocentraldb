# Browser storage: the checklist for the device checks

This is the checklist for the checks a browser on a real device has to show, which Playwright cannot: Safari's seven-day deletion, `persist()` and the home-screen app, eviction on Android, and the Firefox prompt. Each check fills one or more "unknown" rows of the findings table in [browser-storage.md](browser-storage.md#findings). One person can run them all; the Safari checks need a second visit after eight days.

## Before you start

- **The probe page.** Open it at <https://nocentraldb.github.io/nocentraldb/>. The workflow `browser-storage-pages` publishes it to GitHub Pages; it is not published until the owner enables Pages for the repository (Settings → Pages → Source: GitHub Actions) and starts the workflow on `main` in GitHub Actions. The deploy run shows the address; if it differs from the one above, use the one it shows. Without Pages: download the artifact `browser-storage-probe` from the latest run of the `browser-storage` workflow, unzip it, and put the folder on any static HTTPS host that serves `.wasm` files as `application/wasm`. OPFS needs HTTPS: a phone cannot use `http://` on a local network.
- **What the page does.** It reads its data as soon as it opens and shows the result under "Read on load". Write stores the current time in SQLite (on OPFS) and in IndexedDB. Storage info shows `persisted()`, `estimate()` and the browser. Request persist() asks for persistent storage and shows the answer.
- **One origin for all checks.** Storage, the seven-day counter and a `persist()` grant belong to the origin (scheme, host and port), not the page or its path. Every check on the Pages address shares one origin, `https://nocentraldb.github.io`, and a different path would not change that. A browser, a home-screen app on iOS and a Dock app on macOS each keep their own storage, so the checks below use those to keep apart.
- **During the eight days, do not open the probe page in Safari** on that device, in a tab, from history or from a link. Opening it counts as interaction and resets Safari's counter. Use Safari on other sites on at least seven of the eight days, because the rule counts days of Safari use, not calendar days.

## What to record

For each check, add an entry to the [manual check log](browser-storage.md#manual-check-log) in `docs/browser-storage.md`:

- the date of each step, the device, and the OS and browser versions (Storage info shows the browser's user agent);
- what Storage info showed: `persisted`, `estimate`;
- what Request persist() answered, and whether a prompt appeared;
- what "Read on load" or Read showed: for SQLite and for IndexedDB, the stored time, `null` (lost), or an error.

Then set the row's finding to what you saw, and its source to `measured: manual check <date>` (the date of the log entry's heading). A photo or screenshot of the page is enough as a note; the log needs only the values.

## Safari on macOS

Fills: "Seven-day deletion of script-writable storage", "… reaches SQLite on OPFS", "`persist()` prevents the seven-day deletion" and "Private window" for Safari (macOS).

1. **Day 0, in Safari** (takes about 3 min). Open the page. Press Storage info, then Write, then Read. Record all three. Close the tab. Do not press Request persist() here: a grant would cover the origin and spoil step 3.
2. **Day 0, as a Dock app** (takes about 3 min). With the page open in Safari, choose File → Add to Dock. Open the app from the Dock. Press Request persist() and record the answer. Press Write, Storage info and Read, and record them. Quit the app.
3. **Day 8, in Safari** (takes about 2 min). Open the page in Safari and record "Read on load" before you press anything. If SQLite shows `null`, the seven-day rule reached OPFS.
4. **Day 8, as a Dock app** (takes about 2 min). Open the app from the Dock and record "Read on load". If `persist()` was denied in step 2, the row "`persist()` prevents the seven-day deletion" stays unknown: say so in the log.
5. **Private window, any day after step 3** (takes about 3 min). Open a private window, open the page, press Storage info and Write, and record them. Close every private window, open a new one, open the page and record "Read on load".

## Safari on iOS, in the browser

Fills: "Seven-day deletion of script-writable storage", "… reaches SQLite on OPFS", "`persist()` prevents the seven-day deletion" and "Private window" for Safari (iOS).

1. **Day 0** (takes about 3 min). Open the page in Safari. Press Storage info, Request persist(), Write and Read, and record them. Close the tab.
2. **Day 8** (takes about 2 min). Open the page in Safari and record "Read on load" before you press anything. If `persist()` was granted on day 0 and SQLite still holds the time, record that `persist()` prevented the deletion; if it was denied, that row stays unknown. A separate check without `persist()` would need a second origin; ask pm for one if `persist()` was granted.
3. **Private window, any day after step 2** (takes about 3 min). As step 5 for macOS, with a private tab.

## Safari on iOS, as a home-screen app

Fills: "Add to Home Screen prevents the seven-day deletion" for Safari (iOS). Run it on day 0 together with the browser check above; the app keeps its own storage, apart from Safari's.

1. **Day 0** (takes about 3 min). Open the page in Safari, tap Share → Add to Home Screen, and open the app from the home screen. Press Storage info, Request persist(), Write and Read, and record them, including whether `persist()` was granted. Close the app.
2. **Day 8** (takes about 2 min). Do not open the app in between. Open it from the home screen and record "Read on load".

## Chrome on Android

Fills: "Eviction under storage pressure" for Chrome (Android). Edge on Android fills its row the same way, with Edge instead of Chrome. Use a phone whose data you can spare; filling the storage can make other apps misbehave until you free it again.

1. **Write** (takes about 3 min). Open the page in Chrome. Press Storage info, Write and Read, and record them. Do not press Request persist(). Close the tab.
2. **Fill the storage** (takes about 20 min). Copy large files, for example videos, to the phone until Android warns that storage is low, and then a bit more. Note the free space it reports.
3. **Read** (takes about 2 min). Open the page in Chrome and record "Read on load" and Storage info.
4. **Free the storage** (takes about 5 min). Delete the files from step 2.

If the data survives step 3, record that, with the free space you reached: the browser may evict only at a lower level, so the result is "kept at <free space> free", not "never evicted".

## Firefox

Fills: "`persist()`" and "Private window" for Firefox. Desktop Firefox on any system.

1. **The prompt** (takes about 3 min). Open the page. Press Request persist(). Record whether a prompt appears and what it says. Answer Allow, press Storage info and record `persisted`.
2. **Block** (takes about 3 min). In Firefox's site settings for the page (the padlock in the address bar), clear the permission "Store data in persistent storage" and reload. Press Request persist() again, answer Block, and record the answer.
3. **Private window** (takes about 3 min). As step 5 for macOS, with a private window.

## Any browser: clearing the cache in settings

Fills: "Survives clearing the cache in the browser's settings, keeping site data". Takes about 3 min per browser.

1. Open the page, press Write and Read, and record them.
2. In the browser's settings, clear only cached images and files, and keep cookies and site data.
3. Open the page and record "Read on load".
