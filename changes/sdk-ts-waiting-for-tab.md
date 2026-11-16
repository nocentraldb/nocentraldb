---
type: feature
tickets: [NOC-355]
components: [sdk-ts]
---
## Problem
In Safari, a write from the visible tab waits while the tab that owned the database is hidden and frozen and still holds its storage. The write runs once that tab runs again, but until then the app saw only a pending write and could not tell the user why nothing was saved.

## Use
`db.durability` reports `waitingForTab: true` once this tab has waited 1 s for another tab of the app, and `false` when that tab runs again and the waiting writes go through. Show the user why nothing is saved yet:

```ts
db.durability.subscribe(({ waitingForTab }) => {
  otherTabNotice.hidden = !waitingForTab;
});
```

docs/browser-durability.md has a text for it ("Another tab is open"). A tab whose own `nc.open` runs while the owner is frozen waits in `nc.open` and cannot report it yet (NOC-356).
