# RFC 0012: Several tabs on one browser database, without one owning tab

REQUEST FOR COMMENTS · NOVEMBER 2026 · Ticket NOC-357

This RFC lets a browser database do without the tab that owns it. Today one tab's worker holds the database for the whole session, and every other tab reaches it through that tab ([RFC 0006, "The browser"](0006-local-view.md#the-browser)). On iOS, WebKit freezes a hidden tab without a reliable warning, and a frozen owner keeps its lock and its storage. The other tabs then wait for it: their writes (NOC-352, reported as waiting by NOC-355), a new tab's `nc.open` (NOC-356), and the relay's updates, since the owner syncs.

A research note shared with the owner on 2026-11-16 ("Local-First SQLite auf dem iPhone: Multi-Tab-Strategien") proposes another design: every tab has its own connection, which holds the storage only for the length of one transaction, and every write goes first into an outbox in IndexedDB, which a frozen tab cannot block. This RFC checks the note's claims against their sources, measures the two mechanisms in WebKit, works out what they mean for a database whose core signs, validates and syncs, and lists the options and the one the owner chose. [What changes for developers](#what-changes-for-developers) lists every change a developer will see, and [Follow-up tickets](#follow-up-tickets) the work it leads to.

It builds on [RFC 0006](0006-local-view.md), which puts the view on SQLite in a dedicated worker with the `opfs-sahpool` VFS; on [RFC 0002](0002-change-and-log-format.md), whose author sequence makes one member key a single writer; on [RFC 0007](0007-relay.md), whose push is idempotent; and on [RFC 0009](0009-sdk-api.md), which says when a write resolves.

**How to give feedback.** Start a discussion in the category [RFC feedback](https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback), naming RFC 0012 and the section you mean, or open a pull request against this file to change its text. [Giving feedback](README.md#giving-feedback) says how, and what happens to your point.

**Status.** Decided by the owner (Carsten, 2026-11-16): option B, every tab takes the database per transaction and no tab owns it, with the answers in [What changes for developers](#what-changes-for-developers). The outbox is deferred until telemetry gathered with the app's consent shows whether it is needed (NOC-364). Nothing is built yet. RFC 0006 and RFC 0009 still describe the owning tab; the [follow-up tickets](#follow-up-tickets) rewrite them and build the design.

**Out of scope.** Native apps and Node.js, where the database is a file with WAL and several connections already ([RFC 0006, "One writer, many readers"](0006-local-view.md#one-writer-many-readers)). Private windows, which do not open a database at all ([RFC 0006, "What the SDK reports"](0006-local-view.md#what-the-sdk-reports)). Eviction of the copy.

## Summary

| Question | Answer |
| --- | --- |
| Does the note's diagnosis hold? | Yes. WebKit does not reliably fire `visibilitychange` before it suspends a page or when a tab is closed from the tab overview (WebKit bugs 207256 and 205942, both open), a suspended page keeps its Web Locks, and WebKit frees a worker's OPFS access handles only once its page runs again (NOC-352). |
| Does a VFS from the note solve it? | Not as the note says. wa-sqlite's `OPFSCoopSyncVFS` keeps its handles after a transaction and gives them up only when another tab asks, which a frozen tab cannot answer: TanStack DB #1883 is that hang. `opfs-wl` needs COOP and COEP, and did not open a database in WebKit in our spike. Both are a second SQLite module behind JavaScript, which RFC 0006 rejected. |
| What does solve most of it? | Taking the database per transaction with the VFS we already ship: `opfs-sahpool`, resumed before and paused after each transaction under a Web Lock. Then a frozen tab blocks the others only if it froze inside a transaction. Measured in WebKit: it works across two tabs. A small write costs about 8 ms through our Rust VFS (5 ms through the official build) against 1.5 ms with the handles held. |
| Does the outbox work? | Its mechanics do: in all three engines a tab wrote to IndexedDB while another tab held the database, and the replay applied each entry exactly once, a crash between replay and delete included. Its meaning for NoCentralDB is the hard part: an entry cannot be a signed change, so it is an intent that is signed later, which changes what a resolved write promises. |
| What is the cost for the core? | Large. The core keeps each space's log state, tips, membership checks, clock and person keys in memory. A core that holds the database only per transaction has to bring that memory up to date each time another tab has written. |
| Which option? | Option B, chosen by the owner (Carsten, 2026-11-16): a lock per transaction on `opfs-sahpool`, one tab syncing at a time, no outbox. Keeping one owning tab (A) is not taken. The outbox (C) is deferred until telemetry, gathered with the app's consent, shows whether it is needed (NOC-364). |

## The problem in NoCentralDB today

One tab owns the database (NOC-177). Its worker installs the `opfs-sahpool` pool, which takes an OPFS synchronous access handle for every file of the pool, and keeps them until the tab gives the database up. The other tabs send their calls over a `BroadcastChannel`, and the owner's core runs them, syncs with the relay and passes every event on ([RFC 0006, "The browser"](0006-local-view.md#the-browser)).

Two changes already soften a frozen owner. The owner gives the database up when its page is hidden (NOC-333), and its worker closes the database, pauses the pool and only then releases its lock, so the next worker finds the handles free (NOC-352, `ncdb_core::view::browser::release`). Both rely on the page running when it is hidden. On iOS it often does not:

- **`visibilitychange` may not fire before the page is suspended.** WebKit bug 207256, "visibilitychange event does not get always fired on background but it may on foreground", open since 2020. A WebKit engineer comments: "Likely related to process suspension on iOS", and that the event then fires only when the process resumes.
- **Closing a tab from the tab overview fires nothing.** WebKit bug 205942: "If the user instead closes the tab, then no `visibilitychange` event is fired (neither is `pagehide`, `beforeunload`, or `unload` …)", open since 2020.
- **A suspended page keeps its locks.** componentor/fs's notes on Safari: a suspended leader keeps "holding the `vfs-server` Web Lock (a suspended process does not release locks)". Whether WebKit also keeps the OPFS access handles of a suspended worker is not stated there; NOC-352 measured that WebKit frees a worker's handles only once its page runs again.

So a write in another tab waits (NOC-352; NOC-355 reports it as `waitingForTab`), a new tab's `nc.open` waits (NOC-356), and nothing syncs, until the owner's page runs again.

## What the research note claims, checked

Each claim was checked against the source the note gives. "Holds" means the source says it; "corrected" means the source says something narrower or different; "not verified" means no source we could reach says it.

| Claim in the note | Result | What the source says |
| --- | --- | --- |
| SQLite-WASM has a VFS `opfs-wl`, since 3.53, that needs `SharedArrayBuffer`, so COOP/COEP. | Holds, partly by inference | [sqlite.org, persistence.md](https://sqlite.org/wasm/doc/tip/persistence.md), "OPFS with Web Locks": "Added in 3.53.0", it "works identically to the "opfs" vfs" except that it "uses Web Locks for locking" and "requires Atomics.waitAsync()". The page says `opfs` needs `SharedArrayBuffer` and the headers; it does not say so of `opfs-wl` separately. |
| `opfs-wl` holds the file only for a transaction. | Not verified | The page does not say it for `opfs-wl`. Of `opfs` it says "sqlite3 only acquires a write-mode handle when the database API requires a lock". |
| Unlock-ASAP is "up to 4× slower" on I/O-heavy work. | Corrected | The page, "Unlock-ASAP Mode": "up to 400% run time increase in I/O-heavy benchmarks", which is up to five times the run time. It calls the mode a "last-ditch measure" and, as of 2026-03, "never proven to be especially necessary". |
| wa-sqlite's `OPFSCoopSyncVFS` needs no COOP/COEP. | Holds | [wa-sqlite's VFS table](https://github.com/rhashimoto/wa-sqlite/tree/master/src/examples): "No COOP/COEP requirements ✅". |
| `OPFSCoopSyncVFS` takes the handle only for a transaction. | Corrected | [Its source](https://github.com/rhashimoto/wa-sqlite/blob/master/src/examples/OPFSCoopSyncVFS.js) and README: it "closes them lazily … to support multiple connections while retaining performance with a single connection". After an unlock it keeps the handles until another connection asks over a `BroadcastChannel`. A frozen holder cannot answer. |
| PowerSync recommends `OPFSCoopSyncVFS` for Safari and iOS. | Holds, conditionally | [PowerSync's storage docs](https://docs.powersync.com/client-sdks/reference/javascript-web-storage): "If your app has a significant number of Safari users, use OPFSCoopSyncVFS until a known Safari issue is fixed." [Docs PR 66](https://github.com/powersync-ja/powersync-docs/pull/66) (merged 2025-01-28) says multiple tabs do not work in Safari with their default VFS. |
| powersync-js PR 786 adds timeouts against deadlocks. | Holds, narrower | [PR 786](https://github.com/powersync-ja/powersync-js/pull/786), "[Fix] OPFS Multitab Deadlocks", merged 2026-01-05: "a 10-second timeout" for opening database connections. Its main fix is reopening a closed connection elsewhere, and it says Safari freezing is "not solved entirely". |
| TanStack DB #1883: a frozen tab blocks open. | Holds | [Issue 1883](https://github.com/TanStack/db/issues/1883), "openBrowserWASQLiteOPFSDatabase() never settles while a frozen tab holds the database", with `OPFSCoopSyncVFS`, reproduced by freezing a tab over Chrome DevTools; a second report shows the same hang on Mobile Safari 26.6, with two frames of one app instead of a frozen tab. Closed by PR #1936, which the issue does not describe. |
| componentor/fs documents Safari's sync-handle limits. | Holds | [SAFARI-SYNC-LIMITATIONS.md](https://github.com/componentor/fs/blob/main/SAFARI-SYNC-LIMITATIONS.md): WebKit's handles have "always been strictly exclusive", and `createSyncAccessHandle({ mode })` with `"readwrite-unsafe"` is "Chrome 121+ only". |
| WebKit 283959: no `readwrite-unsafe`. | Holds | [Bug 283959](https://bugs.webkit.org/show_bug.cgi?id=283959), "Support readwrite-unsafe mode for createSyncAccessHandle()", status NEW. |
| WebKit 207256 and 205942: no reliable events on suspend and on close. | Holds | Quoted in [The problem in NoCentralDB today](#the-problem-in-nocentraldb-today). Both open. |
| IndexedDB's durability defaults to relaxed; `strict` flushes. | Holds for Chrome | [Chrome's blog](https://developer.chrome.com/blog/indexeddb-durability-mode-now-defaults-to-relaxed): the default moves to `relaxed` "from Chrome 121"; `strict` "instructs the OS to flush changes to disk before issuing the `complete` event", yet "does not ensure that changes are actually written immediately to disk". MDN calls the option "a hint to the user agent". |
| Safari honours `durability: "strict"`. | Not verified | No source says whether WebKit flushes for `strict`. Our spike shows only that WebKit accepts the option and reports `tx.durability === "strict"`. |
| WebKit's storage policy ([webkit.org/?p=14403](https://webkit.org/blog/14403/updates-to-storage-policy/)). | Holds, on quota and eviction | "Updates to Storage Policy", 2023-08-10: every origin is best-effort by default, and WebKit grants `persist()` "based on heuristics like whether the website is opened as a Home Screen Web App". It says nothing about IndexedDB durability. NOC-33 measured that branded Safari on iOS denied `persist()` even in a home-screen app ([RFC 0006, "What a browser does with the copy"](0006-local-view.md#what-a-browser-does-with-the-copy)). |
| Safari fires the Page Lifecycle `freeze` event. | Not verified | The source we tried (MDN's `freeze` event page) no longer exists. The design below does not rely on `freeze`. |

## What the spike measured

The spike ran on 2026-11-16 in the Playwright 1.63 image the CI uses (`mcr.microsoft.com/playwright:v1.63.0-noble`), with two tabs of one origin in a normal window on a fresh profile (NOC-344), with `@sqlite.org/sqlite-wasm` 3.54.1, the official build whose `opfs-sahpool` our Rust `sqlite-wasm-vfs` ports. Its code is in [Appendix: the spike](#appendix-the-spike). It is not merged and not run by the gates.

| Check | WebKit | Chromium | Firefox |
| --- | --- | --- | --- |
| `opfs-sahpool`, handles held, one small write transaction | 1.6 ms | 1.6 ms | 5.0 ms |
| The same, the pool resumed before and paused after each transaction under a Web Lock: resume + transaction + pause | 1.7 + 2.3 + 1.0 = 5.0 ms | 2.2 + 2.0 + 1.3 = 5.5 ms | 11.1 + 6.7 + 9.8 = 27.6 ms |
| Two tabs writing 100 each, interleaved, per transaction | all 600 rows present, about 5 to 6 ms each | all 600 rows, about 6 to 7 ms each | all 600 rows, about 25 ms each |
| One tab stops inside a transaction and holds on; the other asks for the lock with a 1 s limit | waits 1 s, then `TimeoutError` | the same | the same |
| Meanwhile the waiting tab writes 50 entries to IndexedDB with `durability: "strict"` | 1.1 ms each; `tx.durability` reads `"strict"` | 0.8 ms; `"strict"` | 0.7 ms; `"strict"` |
| The stopped tab closes; the other replays the outbox, "crashes" before deleting it, and replays again | 50 applied, then 0; the stopped transaction rolled back | the same | the same |
| One tab holds a second Web Lock and never lets go; the other requests it with `steal: true` | the stealer runs at once; the holder's request ends with `AbortError` | the same | the same |
| One tab commits; the other then reads SQLite's file change counter (header bytes 24 to 27) | rose by one per commit of the other tab | the same | the same |
| Our Rust port (`sqlite-wasm-vfs` 0.3.0 through `ncdb_core::view::browser`), handles held, one small write transaction | 1.5 ms | 1.5 ms | 5.1 ms |
| The same, with `browser::install` before and `browser::release` after each transaction (one tab) | 4.9 + 2.2 + 1.2 = 8.4 ms | 8.4 + 2.3 + 2.2 = 12.9 ms | 16.5 + 4.5 + 10.2 = 31.2 ms |
| `opfs-wl` with COOP/COEP, two tabs with an open connection each | installs (`crossOriginIsolated`, `Atomics.waitAsync` present), but opening a database did not finish within 120 s | works; 100 writes per tab at the same time took about 255 ms per write | not run |
| `opfs-wl`: one tab stops inside a transaction; the other writes with `busy_timeout = 3000` | not reached | the write had not returned when the run hit its 120 s limit | not run |

What the spike shows: the pool of the VFS we already use can be taken per transaction, in all three engines, with a bounded wait, at about three times the cost of a small transaction through the official build and five to nine times through our Rust port, whose `install` takes the pool back more slowly (4.9 ms in WebKit, 8.4 ms in Chromium, 16.5 ms in Firefox). It goes through `sqlite_wasm_vfs::sahpool::install` each time before it resumes; resuming the pool it keeps may be cheaper, which was not tried. A tab can tell from the file's header that another tab committed, and a tab can take a Web Lock from a holder that does not let go. The outbox's write and its exactly-once replay work. `opfs-wl` is not usable in WebKit as tested.

What it does not show:

- **A real freeze.** Playwright cannot freeze a WebKit page. "A tab that stops inside a transaction" is a tab whose code waits forever with the lock and the handles, and "the tab is gone" is a closed page. Whether iOS frees a frozen worker's handles when it discards the tab is unknown. Only a real iPhone shows it.
- **The Rust port across tabs.** Our Rust port was timed in one tab only, from a local example that is not committed; the locks, the outbox and the two tabs ran with the official JavaScript build.
- **Power loss.** Neither `synchronous = FULL` on `opfs-sahpool` nor a `strict` IndexedDB transaction can be checked without cutting a real device's power. Killing a tab is not the same.
- **Pool size.** A resume takes every handle of the pool again; the spike's pool had the default six slots. A database with more files, or several databases on one origin, costs more per resume.

## What it means for NoCentralDB

The note's design was written for an app whose SQLite is the whole database. NoCentralDB's core is more than its SQLite file, and four of its properties decide what the design can be.

### A change is sealed by the one tab that holds the database

A change carries its author's sequence, the heads it depends on and the author's clock, all read from the log ([RFC 0002, "Author and author sequence"](0002-change-and-log-format.md#author-and-author-sequence)). Two tabs share one member key, since they open the same database. If two tabs sealed a change each without seeing the other's, both would carry the same sequence: an equivocation, which every member records and the SDK reports as "one member key now writes from two places". So only the tab that holds the database may seal a change. An outbox entry written by a tab that does not hold it cannot be a signed change. It can only be an **intent**: the app's write function's records, sealed later by whoever holds the database.

An intent has consequences a developer would see:

- **No change id yet.** `write` resolves today with `result.change`, the change id, which is a hash of the sealed change ([RFC 0009, "One write, one change"](0009-sdk-api.md#one-write-one-change)). A queued intent has none. Object ids do not depend on the log and can be drawn at once, so `result.ids` stays.
- **The app does not read its own write at once.** RFC 0006 promises that the member reads its write from the view at once ([The member's own writes](0006-local-view.md#the-members-own-writes)). A queued intent is not in the view until it is replayed. Laying queued intents over query results would be a second read path beside the view, which principle 6 rules out.
- **A refusal arrives late.** The tab that writes the intent can check it against the model, which every tab holds. It cannot check it against the membership state of the log it does not hold: a write into a space the member has since left, or under a right a revocation took, is refused only at replay. Principle 3 then needs the refusal to reach the app, after the write had resolved.
- **Durability has a third kind of change.** `db.durability` counts changes in the log that the relay has not confirmed ([RFC 0006, "What the SDK reports"](0006-local-view.md#what-the-sdk-reports)). A queued intent is neither: it exists only in this browser's IndexedDB, and an eviction takes it with the rest.

### The core keeps state in memory

The Rust SDK's `Db` keeps, per space, the membership checks with their space keys, the clock, and the person keys fetched from the relay; the core's `View` keeps each space's log with its states and tips (`crates/nocentraldb/src/db.rs`, `crates/ncdb-core/src/view/mod.rs`). Today that memory is right because one core is the only writer for the session. With a lock per transaction, another tab may have written since this tab last held the database. Each time a core takes the database, it must first learn whether the file changed, and if it did, bring its memory up to date: the new log rows, their states, the control changes they add to the checks, the clock.

Reading everything again costs as much as an open, which grows with the log. An incremental refresh reads only the log rows after the last one this core saw. It needs a cheap test for "the file changed since I last held it". `PRAGMA data_version` does not serve, because it compares within one connection, and a core that pauses the pool closes its connection after every transaction. SQLite's file change counter, four bytes of the database header that every commit raises, does: in the spike it rose by one with each commit of the other tab, in all three engines. The refresh touches the core's view and the SDK's `Db` together. This is the largest piece of work in every option but A, and it is work in the core, which is the same on every target (principle 7).

### One tab syncs

One relay connection per tab would push every unconfirmed change once per tab. That is harmless, since a push is idempotent and the relay answers `duplicate` ([RFC 0007, "Push"](0007-relay.md#push-append-signed-encrypted-changes)), but it multiplies traffic and pulls. RFC 0007 says "a member opens one connection" and does not say what the relay does with two at once. So one tab syncs at a time, chosen with a second Web Lock, `nocentraldb/<name>/sync`, and folds what it pulls by taking the database lock per page.

A frozen sync tab keeps that lock too. Unlike the storage, a Web Lock can be taken from its holder: `navigator.locks.request(name, { steal: true }, …)` gives the lock to the caller at once, and the holder's request ends with an `AbortError`. The spike showed both in WebKit ("Lock was stolen by another request"), Chromium and Firefox, with a holder that never let go; it did not freeze the holder. So the visible tab may steal the sync lock when the sync tab does not answer. A frozen tab that wakes after losing it may still push once before it notices, which the relay answers with `duplicate`. Stealing does not help the database itself: the OPFS access handles stay with the frozen worker whoever holds the lock.

### Every wait has a limit

A tab that waits for the database lock waits at most a fixed time, then reports it, as NOC-355's `waitingForTab` does for a write today. What it reports changes: with a lock per transaction, a tab waits only while another tab is inside a transaction, so a long wait means that tab froze inside one.

What the user sees while a tab waits is not this RFC's to design. The SDK gets an optimistic UI layer, in the style of SWR and TanStack Query, that shows the last state at once and shows that it refreshes (Carsten, 2026-11-16); its own RFC designs it (NOC-363). This RFC only gives that layer what it needs: a wait that ends within a known time, and a report while it lasts.

## The options

### A. Keep one owning tab (not taken)

Today's design, with NOC-333 (give up on hide), NOC-352 (free the handles before the lock), NOC-355 (`waitingForTab` for a write) and NOC-356 (the same for `nc.open`).

- **Gives:** nothing new to build beyond NOC-355 and NOC-356. The core and its memory stay as they are.
- **Costs:** on iOS, when the owner freezes without a hide event (bugs 207256 and 205942), every other tab's writes, opens and sync wait until it runs again. NOC-355 and NOC-356 tell the user so; they do not shorten it.
- **Why not:** a user does not know which tab is "the right one", expects every tab to sync while it is online, and "another tab is asleep" would make every app look broken (Carsten, 2026-11-16).

### B. A lock per transaction on `opfs-sahpool`, one tab syncing

Every tab runs its own worker and its own core on the same file. A core takes the Web Lock `nocentraldb/<name>` and resumes the pool only for one transaction (a write, a read, a fold page, a rebuild step), brings its memory up to date if the file changed, runs, commits, pauses the pool and releases the lock. One tab syncs, under `nocentraldb/<name>/sync`, which the visible tab may steal. A tab tells the others over the existing `BroadcastChannel` when it committed, so their subscriptions fire.

- **Gives:** a frozen tab blocks the others only if it froze inside a transaction, which lasts milliseconds. `nc.open` no longer waits for another tab outside that window, so NOC-356's case mostly goes away. Sync continues in the visible tab. No new dependency, no COOP/COEP, and the same Rust SQLite on every target (principle 7): `opfs-sahpool`'s `pause` and `resume` are already in our VFS.
- **Costs:** the core's refresh of its memory ([The core keeps state in memory](#the-core-keeps-state-in-memory)). Through our Rust VFS, a small write costs about five times what it does today in WebKit, six in Firefox and nine in Chromium (8.4, 31.2 and 12.9 ms); folds and rebuilds amortise it over a page, and a faster resume may lower it. The SDK's tab layer (`tabs.ts`) is rewritten: no owner, no forwarding of calls.
- **Remaining risk:** a tab frozen inside a transaction holds the handles until it runs again or is discarded; the others' writes wait, reported by `waitingForTab`. How often iOS freezes a worker inside a transaction of a few milliseconds is unknown.

### C. B plus an outbox in IndexedDB (deferred until telemetry, NOC-364)

As B. In addition, a write that cannot take the database within its limit goes into an outbox in IndexedDB, `durability: "strict"`, as an intent. The next core that holds the database seals and folds the open intents in one transaction, records each intent's id in a table of the same SQLite transaction, so a replay after a crash applies none twice (shown in the spike), and then deletes them from IndexedDB.

- **Gives:** a write never waits for another tab; it is kept in this browser at once, even in the remaining risk of B.
- **Costs:** everything in [A change is sealed by the one tab that holds the database](#a-change-is-sealed-by-the-one-tab-that-holds-the-database): a write resolves without a change id, the app does not read it at once, its refusal can arrive after it resolved, and durability needs a count of queued intents. A second store with its own encoding of the write's records, versioned with the model. Whether WebKit flushes a `strict` transaction is unknown.
- **Deferred:** the owner decides on the outbox later, from telemetry the SDK gathers with the app's explicit consent, which shows how often a write waits for a tab frozen inside a transaction (Carsten, 2026-11-16). Its own RFC designs that telemetry (NOC-364).

### D. The note's VFS: `opfs-wl` or `OPFSCoopSyncVFS`

Replace our SQLite with the official build's `opfs-wl`, or with wa-sqlite and its `OPFSCoopSyncVFS`.

- **Gives:** locking that others maintain.
- **Costs:** both are a second WebAssembly module behind a JavaScript API, so the Rust core would call SQLite through JavaScript and the browser would run another SQLite binding than native; RFC 0006 rejected both for that (principle 7). `opfs-wl` needs COOP and COEP on every app's site and did not open a database in WebKit in the spike. `OPFSCoopSyncVFS` keeps its handles after a transaction and frees them only on request, which a frozen tab cannot answer: it has the very hang this RFC is about (TanStack DB #1883). Porting `OPFSCoopSyncVFS`'s approach to our Rust VFS, which RFC 0006 names as the fallback, gives a lazy release that B does not need.

## The chosen option

**B** (Carsten, 2026-11-16). It is built in this order:

1. **The core's refresh,** so that a core that takes the database brings its memory up to date from the file change counter ([The core keeps state in memory](#the-core-keeps-state-in-memory)).
2. **The lock per transaction** in the SDK's tab layer, and **the sync lock** that the visible tab may steal.
3. **A device test on a real iPhone,** from the device checklist ([docs/browser-storage-checklist.md](../browser-storage-checklist.md)): a tab frozen while another writes, a tab closed from the tab overview, a tab frozen mid-sync, and whether iOS frees a frozen worker's OPFS handles when it discards the tab. It measures B's remaining risk.

The outbox (C) waits for the telemetry of NOC-364. The note's VFS (D) is not taken.

The note's fourth point, offering the app as a home-screen app with `navigator.storage.persist()`, needs no decision here: the SDK already asks for persistent storage on open, and NOC-33 measured that iOS denies it even in a home-screen app ([RFC 0006, "What a browser does with the copy"](0006-local-view.md#what-a-browser-does-with-the-copy)). It is advice for developers, not a remedy for frozen tabs.

## What changes for developers

Each of these changes what developers or their users see, so the owner decided each (GENESIS.md, "What has to come back through me").

1. **Option B.** Every tab takes the database per transaction, and no tab owns it (Carsten, 2026-11-16). Keeping one owning tab (A) and the note's VFS (D) are not taken; the outbox (C) is deferred until telemetry (NOC-364).
2. **Every tab opens the database itself** (Carsten, 2026-11-16). Each tab runs its own worker and core; no tab forwards another's calls and there is no owner to hand over to. `db.close()` ends only the tab's own core. A tab whose model lock differs from the file's is still refused with `modelRefused`. RFC 0006's "One tab owns the database" and RFC 0009's "Open and close" are rewritten to say so ([Follow-up tickets](#follow-up-tickets)).
3. **Tabs opened with different relays are refused** (Carsten, 2026-11-16). One tab syncs at a time, and a tab's status names the relay of the tab that syncs ([RFC 0009, "Sync and status"](0009-sdk-api.md#sync-and-status)). A tab whose `nc.open` names another relay than the tabs already open is refused, as a different lock is, so two relays never take turns on one database. The error's name is a public name, which the implementation ticket brings to the owner.
4. **Writes cost more, and that is accepted** (Carsten, 2026-11-16: "we'll see, it is worth the risk"). Through our Rust VFS a small write took about 8 ms instead of 1.5 ms in WebKit, 13 ms instead of 1.5 ms in Chromium, and 31 ms instead of 5 ms in Firefox in the spike, before any work on a faster resume. The cost is measured in practice, in the browser tests of every engine, once the design is built.
5. **`waitingForTab` keeps its name, with a narrower meaning** (Carsten, 2026-11-16). It reports that a write waits because another tab froze inside a transaction, the only case left in which a tab waits for another. NOC-356's `nc.open` wait becomes the same rare case. What the user sees meanwhile is the optimistic UI layer's (NOC-363).
6. **When a write resolves under an outbox** stays open with the outbox (NOC-364). Either `write` waits as it does today and only falls back to the outbox after its limit, or it always queues first. In both, a queued write would resolve without `result.change`, would not be read back at once, and could be refused after it resolved, reported by a new event; and `db.durability` would gain a count of queued intents. Each is a public name for that RFC to bring to the owner.

## Follow-up tickets

This RFC changes no other RFC and no code. The decided design leads to these tickets:

1. **RFC 0006, the browser.** Rewrite "One tab owns the database" and the points under it, the browser's row of the targets table ("One connection, in a dedicated Web Worker") and "One writer, many readers", for a lock per transaction on `opfs-sahpool`, one syncing tab under a stealable sync lock, and the refusal of a different relay. The rejected-stacks table keeps wa-sqlite and `opfs` and gains the reasons found here (lazy release, `opfs-wl` in WebKit).
2. **RFC 0009, the SDK.** Rewrite "Open and close" (every tab opens the database itself; `db.close()` ends only the tab's own core), the `relay` row of "Sync and status" (the syncing tab's relay), the refusal of a tab with another relay and its error name, and the meaning of `waitingForTab`.
3. **The core's refresh.** In `ncdb-core` and the Rust SDK's `Db`: a core that takes the database reads SQLite's file change counter and, when another core committed, brings its log states, tips, checks, clock and person keys up to date from the rows after the last one it saw. Tested with two cores on one file, natively and in wasm32: after the other core wrote, it reads exactly what a freshly opened core reads.
4. **A faster resume in the browser.** `ncdb_core::view::browser::install` goes through `sqlite_wasm_vfs::sahpool::install` before it resumes; resuming the pool it keeps may cost less. Measured before and after in the three engines.
5. **The SDK's tab layer.** Replace the owning tab in `packages/sdk` (`tabs.ts`, the worker and `connect.ts`) with a worker and core per tab: the database lock per transaction with a time limit, the sync lock that the visible tab may steal, a commit notice over the `BroadcastChannel` so other tabs' subscriptions fire, the refusal of a different relay, and `waitingForTab` with its new meaning. The browser tests of NOC-344 in all three engines: two tabs writing interleaved, a tab stopped inside a transaction, a stolen sync lock, a crashed tab whose transaction rolls back, and Chromium's DevTools freeze.
6. **The write cost in practice.** A measurement in the browser tests of every engine, so the cost the owner accepted is known and watched as the design is built.
7. **The device checks.** Rows in [docs/browser-storage-checklist.md](../browser-storage-checklist.md) for a real iPhone: a tab frozen while another writes, a tab closed from the tab overview, a tab frozen mid-sync, and whether iOS frees a frozen worker's OPFS handles when it discards the tab.
8. **The developer pages.** [Getting started in the browser](../getting-started-browser.md) and [Browser apps and durability](../browser-durability.md) say how several tabs behave, and what `waitingForTab` now means.
9. **NOC-356** is rescoped or closed: with every tab opening the database itself, `nc.open` waits for another tab only when that tab froze inside a transaction.

## How it is tested

- **Unit and gate tests** for the core's refresh: a core that holds the database after another core wrote must read exactly what a freshly opened core reads. This runs natively and in wasm32 like the fold tests, with two cores on one file.
- **Browser tests in all three engines** (NOC-344's harness): two tabs writing interleaved, a tab stopped inside a transaction (as in the spike), a stolen sync lock, a crashed tab whose transaction rolls back.
- **Chromium's DevTools freeze** (`Page.setWebLifecycleState`, as TanStack DB #1883 used) for a real freeze in one engine. It does not reproduce iOS.
- **A real iPhone,** by hand, from the device checklist: a hidden tab frozen while another writes; a tab closed from the tab overview; a tab frozen mid-sync. Whether iOS honours a `strict` IndexedDB transaction on power loss cannot be tested reliably by hand; it stays a stated unknown.

## Agreement with GENESIS.md

| Promise | How this RFC keeps it |
| --- | --- |
| Principle 2: every copy converges. | Only the tab that holds the database seals a change, so two tabs never equivocate under one member key. |
| Principle 3: nothing is lost silently. | Option C's intents are counted and a late refusal is reported; option B loses nothing a resolved write promised. A transaction a tab did not finish rolls back, as the spike showed. |
| Principle 4: say plainly what it does not do. | The remaining risk of B, the unknowns of WebKit (frozen handles, `strict`), and what Playwright cannot show are named. |
| Principle 6: apps read only from the computed view. | Queued intents are not laid over query results. |
| Principle 7: the same core on every target. | Option B keeps the Rust SQLite and our `opfs-sahpool`; option D, which would fork the binding, is not taken. |
| "Runs on any device … Safari." | The design targets WebKit's suspension, measured in WebKit where Playwright can, and leaves the rest to a real device. |

## Appendix: the spike

Served with and without COOP/COEP by a small Node.js server, driven by Playwright with two pages of one persistent context per engine. The worker, shortened to the parts the results come from:

```js
import sqlite3InitModule from "/sqlite/index.mjs"; // @sqlite.org/sqlite-wasm 3.54.1
const sqlite3 = await sqlite3InitModule();
let pool;

/** Runs fn while holding the db's Web Lock, waiting at most `wait` ms for it. */
const locked = (wait, fn) =>
  navigator.locks.request("spike/db", { signal: AbortSignal.timeout(wait) }, fn);

/** opfs-sahpool, lock per transaction: take the handles, run, give them back. */
const sahTx = (wait, body) => locked(wait, async () => {
  if (!pool) pool = await sqlite3.installOpfsSAHPoolVfs({ name: "spike-sah", directory: "/spike-sah" });
  else await pool.unpauseVfs();
  const db = new pool.OpfsSAHPoolDb("/spike.sqlite");
  try { db.exec("PRAGMA synchronous=FULL"); db.exec("BEGIN IMMEDIATE"); body(db); db.exec("COMMIT"); }
  finally { db.close(); }
  pool.pauseVfs();
});

/** An outbox entry: one IndexedDB transaction, durability "strict". */
const put = (idb, entry) => {
  const tx = idb.transaction("outbox", "readwrite", { durability: "strict" });
  tx.objectStore("outbox").put(entry);
  return new Promise((ok, no) => { tx.oncomplete = ok; tx.onerror = tx.onabort = () => no(tx.error); });
};

/** The replay: each entry's id goes into SQLite in the same transaction as its effect. */
const replay = (entries) => sahTx(5000, (db) => {
  for (const e of entries) {
    if (db.selectValue("SELECT 1 FROM applied WHERE id = ?", [e.id])) continue;
    db.exec({ sql: "INSERT INTO t VALUES (?, ?)", bind: [e.id, "outbox"] });
    db.exec({ sql: "INSERT INTO applied VALUES (?)", bind: [e.id] });
  }
}); // then delete the entries from IndexedDB; a crash before that replays nothing twice

/** A tab that stops inside a transaction, as one frozen there would. */
const stall = () => navigator.locks.request("spike/db", async () => {
  await pool.unpauseVfs();
  const db = new pool.OpfsSAHPoolDb("/spike.sqlite");
  db.exec("BEGIN IMMEDIATE"); db.exec("INSERT INTO t VALUES ('stalled', 'B')");
  await new Promise(() => {});
});
```

The sync lock and the counter:

```js
/** The visible tab takes the sync lock from a holder that does not let go. */
await navigator.locks.request("spike/sync", { steal: true }, async () => {});
// The holder's request rejects: AbortError (WebKit: "Lock was stolen by another request").

/** SQLite's file change counter, read while this tab holds the pool. */
const counter = () => locked(5000, async () => {
  await pool.unpauseVfs();
  try {
    const bytes = pool.exportFile(pool.getFileNames().find((n) => n.endsWith("spike.sqlite")));
    return new DataView(bytes.buffer, bytes.byteOffset).getUint32(24);
  } finally { pool.pauseVfs(); }
}); // the spike exports the whole file; the core would read the header page only
```

The Rust port was timed with a wasm example built like `crates/ncdb-core/examples/browser_view.rs`, which looped over `browser::install().await`, `rusqlite::Connection::open("/rust-spike.sqlite")`, one `BEGIN IMMEDIATE; INSERT …; COMMIT`, the connection dropped, and `browser::release()`.

The `opfs-wl` check opened `new sqlite3.oo1.OpfsWlDb("/spike-wl.sqlite")` in each tab, set `PRAGMA busy_timeout=3000`, wrote 100 rows from each tab at the same time, and then had one tab stop after `BEGIN IMMEDIATE`.

## History

- 2026-11-16 · First published, decided by the owner (NOC-357, #381)
