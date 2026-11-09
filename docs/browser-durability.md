# Browser apps and durability

A NoCentralDB app keeps a full copy of its spaces on the device. In a native app on a desktop or a phone, that copy is the app's own files. In a web app, it lives in the browser's storage for your site, and the browser may delete it. This page says when that happens, what is lost with it, how you reduce the risk, and what you can tell your users.

**Status.** This page is part of a proposal: the SDK interface it describes is proposed in [RFC 0006, Amendment 2](rfc/0006-local-view.md#amendment-2-durability-of-a-browser-copy) and not built yet, and the owner decides its names and the text for your users. The facts about browsers are not a proposal: they come from [How long a browser keeps a web app's copy](browser-storage.md) (NOC-33).

**Where each statement comes from.** Every row below names, in its Source column, the rows of [the findings table](browser-storage.md#findings) it rests on, as `kind: Browser · Behaviour`:

- **measured**: we ran it. Most measurements ran in Playwright's builds of Chromium, Firefox and WebKit on Linux, which are not the branded browsers: Playwright's WebKit is not Safari.
- **documented**: the browser vendor or MDN says so. We have not seen it happen.
- **unknown**: nobody has measured it and no documentation we found says it. The page then says "unknown" and does not guess.

A gate test (`scripts/browser-durability.test.mjs`) fails when a row names a finding that does not exist, or names it with another kind of source. NOC-33 did not measure native apps, so this page makes no claim about them beyond the fact that the browser's rules below do not apply to them.

## Each browser in plain words

Your app's copy is stored with SQLite on the Origin Private File System (OPFS). The member key, which makes this browser a member of its spaces, is stored in IndexedDB of the same site ([RFC 0003, "How the SDK hides keys"](rfc/0003-keys-and-cryptography.md#how-the-sdk-hides-keys)).

| Browser | What happens to the copy | Source |
| --- | --- | --- |
| Chrome and Edge, desktop | The copy survives reloads and browser restarts. By default it is best effort: the browser may delete it when storage runs low. Chrome grants persistent storage without asking the user when it considers the site important, for example because it is installed or bookmarked, and otherwise denies it silently; Playwright's Chromium denied it. | measured: Chromium (Playwright) · Survives a reload; measured: Chromium (Playwright) · Survives a browser restart; measured: Chromium (Playwright) · `persist()`; documented: Chrome, Edge (desktop) · `persist()`; documented: All browsers · Eviction under storage pressure |
| Chrome and Edge, Android | Like every browser, they may delete a best-effort copy when storage runs low. When that happens on a phone is unknown. | documented: All browsers · Eviction under storage pressure; unknown: Chrome (Android) · Eviction under storage pressure; unknown: Edge (Android) · Eviction under storage pressure |
| Firefox | The copy survives reloads and browser restarts, and is best effort by default. When your app asks for persistent storage, Firefox asks the user in a popup and the answer waits for them. A best-effort copy may use the smaller of 10% of the disk and 10 GiB. | measured: Firefox (Playwright) · Survives a reload; measured: Firefox (Playwright) · Survives a browser restart; measured: Firefox (Playwright) · `persist()`; documented: Firefox · `persist()`; documented: Firefox · Quota |
| Safari, macOS | After seven days of Safari use without the user interacting with your site, Safari deletes all of its script-writable storage, IndexedDB included, so the member key is gone. Whether the deletion also reaches the SQLite copy on OPFS is unknown. Safari grants persistent storage by heuristics, such as the site being opened as a web app; whether that stops the seven-day deletion is unknown. Playwright's WebKit, which is not Safari, kept the copy across reloads and restarts. | documented: Safari (macOS) · Seven-day deletion of script-writable storage; unknown: Safari (macOS) · Seven-day deletion reaches SQLite on OPFS; documented: Safari (macOS) · `persist()`; unknown: Safari (macOS) · `persist()` prevents the seven-day deletion; measured: WebKit (Playwright) · Survives a reload; measured: WebKit (Playwright) · Survives a browser restart |
| Safari, iOS and iPadOS, in the browser | The same seven-day deletion as on macOS. Whether it reaches the SQLite copy, and whether persistent storage stops it, is unknown. | documented: Safari (iOS) · Seven-day deletion of script-writable storage; unknown: Safari (iOS) · Seven-day deletion reaches SQLite on OPFS; unknown: Safari (iOS) · `persist()` prevents the seven-day deletion |
| Safari, iOS and iPadOS, as a Home Screen app | A web app added to the Home Screen is not part of Safari and counts its own days of use, which using it resets. WebKit does not expect its data to be deleted. | documented: Safari (iOS) · Add to Home Screen prevents the seven-day deletion |

### The device checks of 2026-11-09

The owner ran the first checks on branded browsers on 2026-11-09 (NOC-60, pull request #74). Their findings rows come with #74; until it is merged, each point below names its entry in [the manual check log](browser-storage.md#manual-check-log) by heading:

- **SQLite on OPFS works in branded Safari**, on iOS 27.0.1 and on macOS 27.2: the time written was read back (measured: manual check 2026-11-09 · Safari 27.0.1 on iOS 27.0.1, iPhone 15; measured: manual check 2026-11-09 · Safari 27.2 on macOS 26.7.2, Apple M1 Pro (2021)).
- **An iOS Home Screen app does not get persistent storage.** `persist()` was denied in Safari on iOS and in a Home Screen app on iOS, and `persisted()` stayed false (measured: manual check 2026-11-09 · Safari 27.0.1 on iOS 27.0.1, iPhone 15; measured: manual check 2026-11-09 · Home-screen app (Safari 27.0.1) on iOS 27.0.1, iPhone 15). A Home Screen copy is therefore best effort, and the SDK reports `best-effort` there.
- **A macOS Dock app gets persistent storage.** `persist()` was granted in a Dock app on macOS, and `persisted()` turned true (measured: manual check 2026-11-09 · Dock app (Safari 27.2) on macOS 26.7.2, Apple M1 Pro (2021)). In Safari on macOS itself it was not asked, and `persisted()` was false (measured: manual check 2026-11-09 · Safari 27.2 on macOS 26.7.2, Apple M1 Pro (2021)). The Dock app has its own storage: it did not see what Safari had written a minute earlier.
- **Firefox on macOS asks with a prompt**, "Allow nocentraldb.github.io to store data in persistent storage?", and Allow grants it (measured: manual check 2026-11-09 · Firefox 156.0.1 on macOS 26.7.2, Apple M1 Pro (2021)).
- **A Firefox private window** has no OPFS. IndexedDB wrote, but a Read in a new private window showed nothing; the log does not say whether the first window was closed first (measured: manual check 2026-11-09 · Firefox 156.0.1, private window, on macOS 26.7.2, Apple M1 Pro (2021)). Whether IndexedDB, and with it the member key, lasts the whole private session in Firefox is therefore unknown.

## What is lost, and when

What the relay has confirmed is never lost with a browser copy: the relay holds it, encrypted, and any member device can fetch it again. What exists only in this browser is lost: the changes this member wrote that the relay has not confirmed yet, and the member key. Without the member key, the browser is no longer that member. The person sets it up again as a new device, paired from a device that still holds their account key, or with their recovery code if this browser was their only device ([RFC 0003, "The account key"](rfc/0003-keys-and-cryptography.md#the-account-key)).

| When | What is lost | Source |
| --- | --- | --- |
| The page reloads, or the browser restarts | Nothing. | measured: Chromium (Playwright) · Survives a reload; measured: Firefox (Playwright) · Survives a reload; measured: WebKit (Playwright) · Survives a reload; measured: Chromium (Playwright) · Survives a browser restart; measured: Firefox (Playwright) · Survives a browser restart; measured: WebKit (Playwright) · Survives a browser restart |
| Your site sends `Clear-Site-Data: "cache"` | Nothing. | measured: Chromium (Playwright) · Survives `Clear-Site-Data: "cache"`; measured: Firefox (Playwright) · Survives `Clear-Site-Data: "cache"`; measured: WebKit (Playwright) · Survives `Clear-Site-Data: "cache"` |
| The user clears only the cache in the browser's settings, keeping site data | Unknown. | unknown: All browsers · Survives clearing the cache in the browser's settings, keeping site data |
| Storage runs low, and the copy is best effort | Everything the site stored, at once: the copy, its unconfirmed changes and the member key. Browsers delete the least recently used site first. | documented: All browsers · Eviction under storage pressure; documented: All browsers · What eviction deletes |
| Safari, after seven days of use without the user interacting with your site | IndexedDB, so the member key, and with it the unconfirmed changes, which this browser can no longer send. Whether the SQLite copy itself is deleted too is unknown. | documented: Safari (macOS) · Seven-day deletion of script-writable storage; documented: Safari (iOS) · Seven-day deletion of script-writable storage; unknown: Safari (macOS) · Seven-day deletion reaches SQLite on OPFS; unknown: Safari (iOS) · Seven-day deletion reaches SQLite on OPFS |
| A private window closes | Everything the site stored in it. | measured: Chromium (Playwright) · Private window: after close (off-the-record context); measured: Firefox (Playwright) · Private window: after close (private browsing mode); measured: WebKit (Playwright) · Private window: after close (ephemeral context); documented: All browsers · Private browsing |

## How to reduce the risk

| What you do | What it changes | Source |
| --- | --- | --- |
| Keep the app online and syncing | A deletion loses only the changes the relay has not confirmed. The SDK sends each change as soon as the relay is reachable, so an app that stays online keeps that number near zero. | documented: All browsers · Eviction under storage pressure; documented: All browsers · What eviction deletes |
| Ask for persistent storage | A persistent copy is spared when storage runs low; in Safari it "might be excluded from eviction". The browser decides: Chrome and Edge grant it to sites they consider important, Firefox asks the user, Safari uses heuristics. In the device checks above, Safari granted it to a macOS Dock app and denied it on iOS, in the browser and in a Home Screen app. Playwright's Chromium and WebKit denied it, and its Firefox waited for the user. Whether it stops Safari's seven-day deletion is unknown. | documented: Chrome, Edge (desktop) · `persist()`; documented: Firefox · `persist()`; documented: Safari (macOS) · `persist()`; documented: Safari (macOS) · Eviction under storage pressure; measured: Chromium (Playwright) · `persist()`; measured: WebKit (Playwright) · `persist()`; measured: Firefox (Playwright) · `persist()`; unknown: Safari (macOS) · `persist()` prevents the seven-day deletion |
| Have users install the app (Home Screen, Dock, or as an app in Chrome) | On iOS, a Home Screen app keeps its own counter of days of use, and WebKit does not expect its data to be deleted. It does not get persistent storage, though: the device checks above found `persist()` denied there, so the copy stays best effort. On macOS, the device checks found a Dock app granted persistent storage. Installing is also one of the signals that make Chrome grant persistent storage. | documented: Safari (iOS) · Add to Home Screen prevents the seven-day deletion; documented: Chrome, Edge (desktop) · `persist()` |
| Offer a native app | A native app keeps its copy in its own files, outside the browser, so none of the browser rules on this page apply to it. | documented: All browsers · Eviction under storage pressure; documented: Safari (iOS) · Seven-day deletion of script-writable storage |

## Private windows

In a private window, the copy lasts at most as long as the private session:

- **Firefox and WebKit** offer no OPFS in a private window: opening it fails, while IndexedDB works (measured: Firefox (Playwright) · Private window: storage (private browsing mode); measured: WebKit (Playwright) · Private window: storage (ephemeral context)). The SDK then keeps the copy in memory and reports it as `temporary`.
- **Chromium** offers OPFS in a private window and deletes it when the window closes (measured: Chromium (Playwright) · Private window: storage (off-the-record context); measured: Chromium (Playwright) · Private window: after close (off-the-record context)). The SDK cannot tell such a window from a normal one, so it reports `best-effort` there.
- **Safari, Chrome and Edge** as branded browsers: unknown (unknown: Safari (macOS) · Private window; unknown: Safari (iOS) · Private window; unknown: Chrome, Edge (desktop) · Private window (Incognito, InPrivate)).
- **Firefox** as a branded browser has no OPFS in a private window either. Whether IndexedDB lasts the whole private session there is unknown ([the device checks of 2026-11-09](#the-device-checks-of-2026-11-09)).

Each private session is a new device: its member key is created in the session and deleted with it (measured: Firefox (Playwright) · Private window: after close (private browsing mode); measured: WebKit (Playwright) · Private window: after close (ephemeral context)), so the person pairs it again every time. When the SDK reports `temporary`, tell your users, with the private-window text below, and suggest a normal window.

## What the SDK tells you

The SDK reports how durable this copy is and how many changes would be lost with it. The names below are proposed in [RFC 0006, Amendment 2](rfc/0006-local-view.md#amendment-2-durability-of-a-browser-copy); the owner decides them.

```ts
const db = await nc.open({ model });

const now = db.durability.current();
// { storage: "best-effort", unconfirmed: 3, oldestUnconfirmedAt: Date, atRisk: true }

const stop = db.durability.subscribe((durability) => showBanner(durability));
await db.durability.requestPersistent(); // asks the browser; Firefox asks the user
```

| `storage` | What it means | What it does not promise |
| --- | --- | --- |
| `persistent` | A native app, or a browser that granted persistent storage. | That the copy survives the user clearing site data, a lost device, or, in Safari, the seven-day deletion (unknown). |
| `best-effort` | The browser may delete the copy when storage runs low, and Safari after seven days without interaction. Most browser copies start here. | Anything about when: the browser decides. A Chromium private window also reports `best-effort`. |
| `temporary` | The copy is in memory and lives at most until the private session ends. | That it survives a reload. |

`unconfirmed` counts this member's changes that the relay has not confirmed; they exist only on this device. `atRisk` is true while `unconfirmed` is above zero and `storage` is not `persistent`. In development, the SDK logs one warning per page load when `storage` is not `persistent`.

## Text for your users

Proposed wording you can show your users, in English. Replace `{appName}` with your app's name and `{count}` with `unconfirmed`. None of it promises more than the findings above.

"Tested" means that the gate checks each text: it names the app, has at most 60 words, uses no word that promises more (never, always, guaranteed, safe, secure, forever, permanent), and has a form for one change and for more. No user has read these texts yet; a test with users is still to come.

### Onboarding

Show it once, when a user first opens your web app and `storage` is `best-effort`.

```text
{appName} keeps your data in this browser, so it works offline. Your browser may delete it: when storage runs low, or in Safari after seven days without using {appName}. Changes that reached the server stay there. Changes not yet sent are lost, and you then set up {appName} in this browser again. Installing {appName} lowers the risk.
```

### Changes not yet saved

Show it while `atRisk` is true and the oldest unconfirmed change is a few minutes old, or when the app goes offline with unconfirmed changes. Use the first form when `unconfirmed` is 1.

```text
1 change in {appName} is saved only in this browser so far. It is sent to the server when you are online. Until then, it is lost if the browser deletes its data.
```

```text
{count} changes in {appName} are saved only in this browser so far. They are sent to the server when you are online. Until then, they are lost if the browser deletes its data.
```

### Private window

Show it when `storage` is `temporary`.

```text
You are using {appName} in a private window. What {appName} stores here is deleted when you close the window, or sooner, together with changes not yet sent to the server. Next time, you set up {appName} again. Use a normal window to keep your data in this browser.
```
