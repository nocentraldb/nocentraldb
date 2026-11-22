# Research: the TipTap binding of RFC 0013 in a browser

RESEARCH NOTE · NOVEMBER 2026 · Ticket NOC-455 · Step 1a of [RFC 0013's build plan](../../rfc/0013-rich-text.md#follow-up-tickets)

**Status.** A spike for the build step that makes the TypeScript binding and `@nocentraldb/tiptap` (step 4, NOC-461). It decides nothing and changes no RFC. Where its findings would change RFC 0013's design, it names them as questions for pm ([Questions for pm](#questions-for-pm)). The spike under [`spike/`](spike/) is outside the pnpm workspace and the gates.

## What the ticket asks, and the short answer

RFC 0013 asks three things of this spike: how the binding avoids echoing its own writes; the latency of a pause-based write between two tabs and between two browsers; and the three measures (widening, content check, guard) with IME input, paste and undo, which F19 did not run.

1. **Echo.** An origin of the binding's own, on everything it applies from the core, is enough: the binding never wrote back what it received. Its own writes come back from the core as a 2-byte update that changes nothing ([B1](#b1-the-origin-check-stops-the-echo)).
2. **Latency.** From the last key to the other tab: 544 ms median, of which 502 ms is the pause. To another browser through the local relay: 597 ms median. While someone types without a pause, a character reaches the other side 1.1 s later at the median and 2.1 s at most, as the 2 s cap sets ([B2](#b2-latency)).
3. **The three measures.** With IME input, paste and undo, they lost nothing, and the guard flagged no ordinary edit ([B3](#b3-the-three-measures-with-ime-paste-and-undo)). But the spike found six things the binding must do that RFC 0013 does not say, and one error in its example code ([B4](#b4-what-rfc-0013-does-not-say-yet)):
   - RFC 0013's example listens with `editor.on("contentError", …)`. That listener never runs, because TipTap's default `onContentError` throws first. The editor then stays paused with no rebuild.
   - The pause writes an IME composition before the person commits it.
   - A rebuild during a composition loses the committed text and leaves the uncommitted text in the document.
   - A rebuild need not cost the undo history.
   - The guard on its own puts the editor in a reload loop and refuses the person's typing.
   - The guard must know the editor's schema before the editor is created.
   - Placeholders need parse rules, or copy and paste drops them.

## The spike

A page that runs TipTap 3.35.0 with StarterKit and `Collaboration`, `@tiptap/y-tiptap` 3.3.1 and `yjs` 13.6.33, bundled with esbuild 0.28.2 and driven by Playwright 1.63.0 in headless Chromium 157. Every version is pinned in [`spike/package.json`](spike/package.json) and locked in `spike/package-lock.json`. [`spike/output.txt`](spike/output.txt) is what it printed, and [`spike/results.json`](spike/results.json) is what it wrote. Two runs gave the same outcomes, with timings a few milliseconds apart.

To run it, build the SDK and the relay from the repository's root, then run the spike in `spike/`:

```sh
node scripts/sdk-browser/build.mjs
cd docs/research/rich-text-browser-binding/spike && npm ci && npm run spike
```

The parts:

- **The binding** ([`page/binding.js`](spike/page/binding.js)) does what [RFC 0013, "An editor: the binding"](../../rfc/0013-rich-text.md#an-editor-the-binding) describes, and nothing more. It keeps a `Y.Doc` with a leased client id, collects the editor's updates and writes them as one change: after 500 ms without an edit, after at most 2 s, and when the page is hidden. It applies what the core holds and the binding does not, computed from the binding's state vector, with an origin of its own. Before it writes, it runs the guard, and it reloads after a refused write.
- **Two cores.** For the echo and the measures, a stand-in core in the page ([`page/core.js`](spike/page/core.js)): a `Y.Doc` that folds every write and tells every subscriber, the writer included. For the latency, the real TypeScript SDK ([`page/sdk-core.js`](spike/page/sdk-core.js)). `nc.richText` is not built yet, so each change is one task object of the test lock, with the Yjs update as base64 in its `title`. The path between tabs and browsers is the SDK's own: `space.create` in the worker, the database on OPFS, `db.changes` in the other tab, and the relay between browsers.
- **The measures** ([`page/measures.js`](spike/page/measures.js), [`page/editor.js`](spike/page/editor.js)) are the NOC-437 spike's, carried over: a schema widened from the document, `enableContentCheck` with a rebuild on `contentError`, and the guard. Each can be turned off on its own.
- **The two apps** ([`page/newer.js`](spike/page/newer.js)) are the NOC-437 spike's. The newer app writes a document with a block `callout` (`tone`), an inline atom `chip` (`ref`), a mark `highlight` (`color`) and an `align` on a paragraph. The older app's editor is StarterKit, which knows none of them. "Lost" below means a part of that document the core no longer holds, read from Yjs alone, with no schema.

How the browser is driven:

- **IME input** goes through the DevTools protocol: `Input.imeSetComposition` for each step of a composition, then `Input.insertText` to commit it. These are the calls Chromium's own tests use; an operating-system IME was not run.
- **Copy and paste** are `copy` and `paste` events with a `DataTransfer`, as ProseMirror reads them. **Undo** is the keyboard's `Control+z` and `Control+Shift+z`.

## B1. The origin check stops the echo

| Case | What the binding wrote | What came back |
| --- | --- | --- |
| The person types twice, with a pause after each | 2 changes, 57 and 35 bytes, only its own client id | after each write, a pull of 2 bytes that changed nothing |
| A collaborator writes three times; origin checked | nothing of the collaborator's | the editor shows the text; 0 updates written back |
| The same, origin not checked | 1 more change, 23 bytes, holding only the collaborator's client id 3 | — |

- **The echo needs only the origin.** The binding applies every pull with an origin of its own, and its `update` listener skips that origin. Its own write comes back from the core as `Y.encodeStateAsUpdate(core, stateVector)`, which holds no new items, so applying it fires no `update` event at all. The binding does not need to know which change was its own.
- **Without the check, the echo is one write per pause, not a loop.** The binding wrote the three updates it had received back as one change, with only the collaborator's client id in it. The core folded it without change, so nothing came back to write again. Under [RFC 0013, "Client ids name their author"](../../rfc/0013-rich-text.md#client-ids-name-their-author), every member would park that operation as "malformed arguments": its items name a client id that is not the author's. So a missing check shows up as parked operations, one per pause, not as a loop.
- **Every pull carries the core's whole delete set.** A diff from a state vector holds every deletion, since a state vector cannot say which deletions the binding has. After 25, 50, 75 and 100 scattered single-character deletions, the no-op pull was 63, 100, 118 and 129 bytes. This is cheap, but the binding could skip a pull when the core tells it about its own write.
- **Undo touches only the person's own edits.** The person typed "Mine.", a collaborator added " Theirs.", the person pressed undo: " Theirs." stayed, and the undo was written as a 6-byte change that only deletes. Redo brought "Mine." back. TipTap's UndoManager tracks only y-tiptap's origin, so text applied from the core is never on its stack.
- **Hiding the page writes at once.** A `visibilitychange` to hidden wrote what was pending, without waiting for the pause.

## B2. Latency

From the last key the writer pressed to the moment the other side's binding holds the text, on one machine with one clock (`performance.timeOrigin + performance.now()`). Ten bursts of a short word, then about 6 s of typing at 10 characters a second without a pause. Each column is median / p90 / max, in ms.

| Between | Last key to the other side | The pause | `space.create` | Write start to the other side | Each character, typing without a pause |
| --- | --- | --- | --- | --- | --- |
| Two tabs of one browser, one database on OPFS | 544 / 550 / 550 | 502 / 502 / 502 | 35 / 40 / 40 | 42 / 49 / 49 | 1125 / 1946 / 2058 |
| Two browsers, each with its own database, through the local relay | 597 / 603 / 603 | 502 / 502 / 502 | 38 / 47 / 47 | 96 / 102 / 102 | 1167 / 1979 / 2096 |

- **The pause is almost all of it.** Between tabs, the SDK's path from `space.create` to the other tab's `db.changes` took 42 ms; through the relay, 96 ms, on the loopback with no network in between. A relay in the cloud adds its round trips to that.
- **Typing without a pause is what D3 decided.** The writes were three of 2 s (20, 20 and 17 keystrokes, 90 to 105 bytes each), so a character waits up to 2 s for its write.
- **A tab closed before the pause still wrote.** The writer typed and its tab was closed at once, with and without `beforeunload`. Both times Chromium fired `visibilitychange` to hidden on the way out, the binding started its write there, and the other tab received the text. Firefox and WebKit were not run ([What this does not show](#what-this-does-not-show)).
- **The headless browser needed help.** A page Playwright had not brought to the front handled typing late or not at all. The spike clicks into the editor of the writing page first, as a person would.

## B3. The three measures with IME, paste and undo

With all three measures on, none of these lost a part of the newer app's document, and the guard flagged nothing:

| Case | What happened |
| --- | --- |
| IME in the paragraph whose `align` the older app does not know | `日本` committed; `align` kept |
| IME right after the chip, and at the end of the highlighted word | `漢` after the chip; `字` took the unknown mark, as typing at a mark's end does |
| Undo of a composition | the composition undone; nothing else touched |
| Paste plain text over a selection that spans the chip | the chip deleted on purpose; the guard let it through, as the RFC wants |
| Copy the chip, the callout or the highlighted word, and paste it | a second copy, with its attributes (`ref`, `tone`, `color`) |
| Paste HTML from another app with `<aside>` and `<mark>` | plain paragraphs; nothing in the document lost |
| Type in the aligned paragraph, undo, redo | `align` kept through all three |
| Delete the chip or the callout, then undo | each came back with its attribute |
| A new kind of node arrives; rebuild on `contentError` | the editor rebuilt once and the person kept typing |
| The same, without a rebuild | typing, paste and IME refused, nothing written; the editor stays paused |

So the widening, the content check with a rebuild, and the guard hold with IME, paste and undo, as F19 found for typing. The table in [B4](#b4-what-rfc-0013-does-not-say-yet) lists what it takes.

## B4. What RFC 0013 does not say yet

### B4.1 RFC 0013's example never rebuilds

RFC 0013's example registers `editor.on("contentError", () => body.rebuild(editor))`. TipTap's `Editor` registers its own `onContentError` option on the same event first, and its default is `({ error }) => { throw error }`. When a new kind of node arrived, that default threw `RangeError: Unknown node type: figure` out of the Yjs observer. The listener never ran, there was no rebuild, and the editor stayed paused for good. Passing the handler as the editor's option worked:

```ts
const editor = new Editor({
  enableContentCheck: true,
  onContentError: () => body.rebuild(editor),
  // …
});
```

This is a correction of the example, not a change of design: the RFC's text already says the editor is rebuilt on `contentError`.

### B4.2 The pause writes an IME composition before it is committed

A composition held for 1.5 s, longer than the pause, was written as a change: `にほん`, the text not yet committed, reached the core. When the person committed `日本`, a second change replaced it. Collaborators would see the uncommitted text, and the log keeps it. ProseMirror updates the document during a composition, so the binding sees updates and starts its pause timer.

A binding could hold its write while `view.composing` is true, or while a composition has not ended, and write at `compositionend`. Holding it at most a set time would keep the 2 s promise. Which of these to take is a question of when a pause is a pause, which D3 decided.

### B4.3 A rebuild during a composition loses the committed text

A new kind of node arrived while a composition (`とう`) was under way. The editor was rebuilt at once. The uncommitted `とう` stayed in the document and was written. The commit `東京` reached no editor and was lost. After the rebuild the editor no longer had focus, so the person had to click back in.

A binding could wait for `compositionend` before it rebuilds, and after the rebuild restore the focus and the selection, kept as a Yjs relative position.

### B4.4 A rebuild need not cost the undo history

RFC 0013 says a rebuild "costs the selection and the undo history". With the editor's own UndoManager, undo after a rebuild did nothing. With one UndoManager that the binding keeps for its `Y.Doc` and passes to each editor it builds (`Collaboration.configure({ yUndoOptions: { undoManager } })`), undo after the rebuild took back the text typed before it. The selection still has to be restored ([B4.3](#b43-a-rebuild-during-a-composition-loses-the-committed-text)).

### B4.5 The guard on its own loops and refuses the person's typing

With the guard but no widening and no content check, the editor deleted the callout, the chip and the highlighted text as it opened. The guard flagged it and the binding refused the write: nothing was lost. Then the reload rebuilt the editor, which deleted them again. In 3 s there were 5 reloads, one per pause, and it does not end.

When the person typed meanwhile, their typing was in the same pending write as the deletion. The guard refused the whole write, so the typing was never written. The editor showed a document without the unknown parts, and let the person edit it.

So the guard keeps the document, but on its own it does not keep the client editing. RFC 0013 says the guard "also catches an app that leaves out the first two measures". The spike shows what follows. After a refused write, the binding could stop reloading and make the editor read-only, reporting the field as not fully shown. Or the binding could refuse to open an editor without the content check.

### B4.6 The guard must know the schema before the editor exists

y-tiptap deletes what the schema lacks while the `Editor` constructor renders the document, before `new Editor(…)` returns. A guard that reads `editor.schema` finds no editor at that moment and lets the deletion through. In the spike's first version it did exactly that. The guard has to be given the schema first, built with TipTap's `getSchema` from the same extensions. So the binding's API needs the editor's extensions, or its schema, before the editor is created.

### B4.7 Placeholders need parse rules

The NOC-437 spike's placeholders had no `parseHTML`. When a person copied a placeholder and pasted it, ProseMirror wrote the placeholder's HTML to the clipboard but could not read it back, and nothing was pasted. With placeholders that render their name and attributes into `data-` attributes and parse them back, the copy kept its type and attributes. `NcKeepUnknown` needs parse rules of this kind.

Pasting into another app, or into the newer app, gives plain text: their schemas do not read these attributes. This spike did not look further at that.

## What this does not show

- **Chromium only.** This container has no Firefox or WebKit, and their IME composition events differ from Chromium's. The IME cases drive the DevTools protocol, which only Chromium has; the other cases would run in Firefox and WebKit if `lib/browser.mjs` launched them. The TypeScript SDK's browser tests already run in all three engines in `browser-view.yml`, so the build ticket's tests can cover them there.
- **No operating-system IME.** The DevTools protocol's composition calls stand in for one. Some IMEs, on Android above all, send other event orders.
- **The core in the latency is a stand-in.** It applies Yjs updates in the page, not Yrs in the worker. A change carries the update as a base64 string in a task object, and its fold is the SDK's fold of an object. The SDK's own work per change is measured, but `nc.richText`'s will differ.
- **No network.** The relay ran on the loopback.
- **No real person.** The typing rhythm is Playwright's: 40 ms between keys in a burst, 100 ms when typing without a pause.

## Questions for pm

The findings that would change RFC 0013's design, each for the owner through pm:

1. **The example's `contentError` handling (B4.1).** Use the editor's `onContentError` option. This corrects the example to do what the text says, and decides nothing. It can go into RFC 0013 in this PR if pm wants.
2. **Writing during an IME composition (B4.2).** Hold the write while a composition is open, and write at `compositionend`, with a cap. Recommendation: hold it, capped at the same 2 s as typing.
3. **Rebuilding during a composition (B4.3).** Wait for `compositionend` before a rebuild, and restore the focus and the selection after it. Recommendation: yes. The rebuild then costs neither, and RFC 0013's "a rebuild costs the selection" goes.
4. **The undo history across a rebuild (B4.4).** The binding keeps one UndoManager per `Y.Doc` and passes it to each editor it builds. Recommendation: yes, and RFC 0013's "costs … the undo history" goes.
5. **What the guard does after it refuses (B4.5).** Either stop and make the editor read-only, reporting the field as not fully shown, or refuse to open an editor without the content check. Recommendation: both. The guard reloads once, then the field is read-only until the app rebuilds the editor with a widened schema. And `space.richText` refuses an editor whose options lack `enableContentCheck`.
6. **The guard's schema and the placeholders' parse rules (B4.6, B4.7).** Build-ticket detail for NOC-461, not RFC text: the binding takes the editor's extensions before the editor exists, and `NcKeepUnknown` parses its placeholders back.
