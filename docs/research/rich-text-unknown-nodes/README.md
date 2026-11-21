# Research: editing a rich-text document whose parts a client cannot interpret

RESEARCH NOTE · NOVEMBER 2026 · Ticket NOC-437 · For RFC 0013's choice D4

**Status.** Research for the owner's decision on RFC 0013's D4, the document vocabulary. RFC 0013 is in PR #422, not yet on `main`. This note decides nothing and changes no RFC. The spike under [`spike/`](spike/) is outside the gates and the pnpm workspace.

## The question and the short answer

The owner's goal (NOC-437): **every client can keep working on a document, even when it cannot interpret parts of it.** GENESIS.md asks the same in principle 3 ("a client keeps data it does not understand yet") and in "Never break an existing client or stored data".

The short answer:

1. **Yjs keeps everything.** A `Y.XmlFragment` stores any element name, attribute and text format. It knows no schema.
2. **The editor binding deletes it, from the Yjs document, not just from the view.** y-prosemirror, and TipTap without `enableContentCheck`, delete an unknown node from the `Y.Doc` as soon as the editor opens the document, even when the editor is read-only, and even when no editor exists and code only converts the fragment into a ProseMirror node. An unknown mark deletes all the text of its paragraph. The deletion is an ordinary Yjs edit, so it syncs and every member loses the content.
3. **TipTap's `enableContentCheck` stops the deletion but also stops the editing.** The client can no longer edit any part of the field. It also misses one case: an attribute the schema does not declare, on a node the schema knows, raises no error, and the next edit of that node removes the attribute.
4. **A client can keep editing if its editor schema is widened from the document before the editor opens.** Each unknown node, mark and attribute gets a placeholder type. With the content check on, and the editor rebuilt whenever a new kind arrives, the spike lost nothing and the older client kept editing. A guard in the binding catches whatever the widening misses.
5. **So D4 does not settle the owner's goal; the binding does.** Under each of A, B and C an older client meets names it does not know. What D4 decides is how the core renders the document for agents and the Rust SDK. The recommendation is **option A with a lossless fallback** ([Recommendation](#recommendation-for-d4)).

## The spike

A Node.js 24 program with `yjs@13.6.33`, `y-prosemirror@1.3.7`, `@tiptap/y-tiptap@3.3.1`, `@tiptap/core`, `@tiptap/starter-kit` and `@tiptap/extension-collaboration@3.35.0`, `prosemirror-*` as they resolve, and `jsdom@26.1.0` for the editor's DOM. Every version is pinned in [`spike/package.json`](spike/package.json) and locked in `spike/package-lock.json`. Run it with `npm ci && npm run spike` in `spike/`. It prints [`spike/output.txt`](spike/output.txt) and writes [`spike/results.json`](spike/results.json). Two runs gave the same output.

Two schemas stand for two versions of one app ([`spike/lib/setup.mjs`](spike/lib/setup.mjs)):

- **The older app** knows `paragraph`, `text` and `bold`.
- **The newer app** adds a block node `callout` (attribute `tone`), an inline atom `chip` (attribute `ref`), a mark `highlight` (attribute `color`), and an attribute `align` on `paragraph`.

The newer app writes a document of five paragraphs, one per unknown kind: `Intro.`, a callout `Careful.`, a centred paragraph `Centered.`, `Some marked text.` with `marked` highlighted, and `See [chip] here.`. The older client receives the update, opens an editor on it, a person types in `Intro.` and then in `Centered.`, and the older client syncs back to the newer one. Each document has a fixed client id, as in RFC 0013's spike. The spike reads what the `Y.XmlFragment` holds from Yjs alone (`toString()`), with no schema, so a ProseMirror step cannot hide a loss.

The editors ([`spike/lib/editors.mjs`](spike/lib/editors.mjs)): a ProseMirror `EditorView` with y-prosemirror's `ySyncPlugin`; the same with y-tiptap's `ySyncPlugin`, with and without `onInvalidContent`; and a TipTap `Editor` with StarterKit and `Collaboration`, with `enableContentCheck` false and true. TipTap's StarterKit knows more than the older schema, but none of the newer app's additions.

## 1. Does Yjs keep what a client does not know?

**Yes.** Before any editor opened the document, the older client's `Y.Doc` held every node, attribute and mark of the newer app (`beforeEditor` in `results.json`: nothing lost). Yjs's `Y.XmlElement` holds a node name, string attributes and children, and its documentation names no schema check [[Yjs: Y.XmlElement](https://docs.yjs.dev/api/shared-types/y.xmlelement)]. A text format is a key-value attribute of a `Y.XmlText` run. Yjs merges and stores them whatever their names. Yrs implements the same types, so the core, which only applies updates, keeps them too (RFC 0013, "Reading": "Storing and syncing a document needs no vocabulary").

A schema enters only where a ProseMirror editor projects the fragment into its own document.

## 2. What the editor binding does with them

| Binding | On open, the Yjs document lost | Person types in a known paragraph | Person types in the centred paragraph | The newer app lost after sync | Reported |
| --- | --- | --- | --- | --- | --- |
| y-prosemirror 1.3.7 | callout, chip, the whole highlighted paragraph | written | written, `align` removed | callout, chip, highlighted paragraph, `align` | nothing |
| y-tiptap 3.3.1, no `onInvalidContent` | the same | written | written, `align` removed | the same | nothing |
| TipTap 3.35.0, `enableContentCheck: false` | the same | written | written, `align` removed | the same | nothing |
| y-tiptap 3.3.1, `onInvalidContent` | nothing | **refused** | **refused** | nothing | 1 error |
| TipTap 3.35.0, `enableContentCheck: true` | nothing | **refused** | **refused** | nothing | 1 `contentError` |
| y-prosemirror 1.3.7, view not editable | callout, chip, the whole highlighted paragraph | (not editable) | (not editable) | — | nothing |
| No editor: `yXmlFragmentToProseMirrorRootNode` or `initProseMirrorDoc` with the older schema (y-prosemirror and y-tiptap) | callout, chip, the whole highlighted paragraph | — | — | — | nothing |
| TipTap 3.35.0, `enableContentCheck: true`, only `align` unknown | nothing | — | written, `align` removed | `align` | nothing |

What the table shows, one finding each:

- **N1. The unknown node is deleted from the Yjs document, not only from the view.** It happens when the editor opens the document, before anyone types. The deletion is a local Yjs transaction, so it goes into the next update and every member applies it. In y-prosemirror, `createNodeFromYElement` calls `schema.node(name, ...)`, which throws for a name the schema lacks, and its `catch` deletes the element's Yjs item. Its comment reads: "an error occured while creating the node. This is probably a result of a concurrent action" [[y-prosemirror `sync-plugin.js`](https://github.com/yjs/y-prosemirror/blob/v1.3.7/src/plugins/sync-plugin.js)]. y-tiptap does the same through `deleteYType` when no `onInvalidContent` is set [[y-tiptap `createNodeFromYElement.ts`](https://github.com/ueberdosis/tiptap/tree/main/packages/y-tiptap)].
- **N2. An unknown mark deletes the whole text of its paragraph.** `attributesToMarks` calls `schema.mark(name)`, which throws for an unknown mark, and `createTextNodesFromYText` then deletes the entire `Y.XmlText`. In the spike, "Some marked text." vanished, not only "marked". The older editor showed an empty paragraph.
- **N3. A read-only editor deletes too.** The projection runs whether or not the view is editable. RFC 0013's reader binding, which refuses a reader's write and reloads, would then refuse this deletion, reload, delete again, and repeat on every open.
- **N4. An attribute the schema does not declare, on a node it knows, is dropped silently and removed on the next edit of that node.** ProseMirror's `NodeType.create` ignores attributes the spec does not declare, so no error is raised, even with `enableContentCheck`. When the person edits that node, `updateYFragment` removes every Yjs attribute that the ProseMirror node lacks ("remove all keys that are no longer in pAttrs"). In the spike, the centred paragraph lost `align`.
- **N5. `enableContentCheck` keeps the content but takes the field away from the person.** y-tiptap then skips what the schema rejects, reports it once, and filters every local transaction that changes the document. TipTap's guide says the editor "shows what it can, pauses editing and sends no changes", and that "editing resumes when a change from another user makes the content fit the schema again" [[TipTap: Invalid schema handling](https://tiptap.dev/docs/guides/invalid-schema)]. In the spike, typing in `Intro.`, a paragraph the older app knows, was refused. The guide also says that the check needs `@tiptap/extension-collaboration` 3.33.0 and `@tiptap/y-tiptap` 3.2.0 or later: older versions delete the content even with the check on.
- **N6. The maintainers know, and plan no passthrough.** Asked about unknown nodes and marks, the Yjs maintainer wrote that y-prosemirror "removes invalid nodes, that don't conform to your schema", advised not to open a document whose schema the editor does not understand and to only extend schemas, and said a passthrough node "is currently not planned" [[discuss.yjs.dev, Dec 2024](https://discuss.yjs.dev/t/handling-unknown-nodes-marks-in-prosemirror-schema/3683)]. Hocuspocus users report the same loss: "if a user receives an update that contains data that is not supported in their schema, it gets deleted" [[hocuspocus #443](https://github.com/ueberdosis/hocuspocus/discussions/443)]. ProseMirror's author: "Schemas can not be updated dynamically"; the advice is to build the schema in advance by scanning the input [[discuss.prosemirror.net](https://discuss.prosemirror.net/t/adding-fallback-extensions-for-unsupported-node-mark-types/4295)].

- **N7. Converting the fragment deletes too, with no editor.** `yXmlFragmentToProseMirrorRootNode` and `initProseMirrorDoc`, in both libraries, go through the same `createNodeFromYElement` and delete what the schema rejects, from the `Y.Doc` they read. A server-side renderer, a preview or an agent tool that only wants the ProseMirror document of a field would delete content. `yXmlFragmentToProsemirrorJSON` reads without a schema and deleted nothing. (The NOC-436 session suggested testing the headless path.)

This confirms RFC 0013's F10, which was found by reading the code, and adds N2 to N5 and N7. RFC 0013 asks whether y-tiptap, with the check on, ever deletes the unknown node during later edits. The answer is that with an unknown node or mark there are no later edits; with only an unknown attribute there are, and they remove it (N4).

## 3. How a client keeps the parts and keeps editing

The spike tried the three ideas the ticket names ([`spike/lib/remedies.mjs`](spike/lib/remedies.mjs)).

### A placeholder type for each unknown kind: widening the schema

ProseMirror has no catch-all node: y-prosemirror looks a node type up by the Yjs element's name. But a schema can be built per document. Before the editor opens, the binding scans the fragment and adds:

- for each **unknown node name**, a node type of that name: an atom, inline or block by where it sits, holding text or blocks as it does, with the attributes it carries declared with a `null` default. The app renders it as a placeholder, a box with the node's name;
- for each **unknown mark name**, a mark type with its attributes;
- for each **undeclared attribute of a known node**, an extra attribute on that type, through TipTap's `addGlobalAttributes`.

| Run | Typed | Lost, older client | Lost, newer client after sync |
| --- | --- | --- | --- |
| y-prosemirror, widened schema | both edits | nothing | nothing |
| TipTap, `enableContentCheck: true`, widened extensions | both edits, no `contentError` | nothing | nothing |

A `null` default means the widened editor writes no value of its own into an attribute. The person can still select a placeholder and delete it whole, as they can delete any block. That is a deliberate edit, not a loss.

**A kind that arrives while the editor is open.** A schema is fixed once the editor exists. When the newer app inserted a `figure` while the older editor was open:

| Run | Result |
| --- | --- |
| y-prosemirror, widened at open | the `figure` is deleted at once |
| TipTap, `enableContentCheck: true`, widened at open, rebuilt on `contentError` | editing pauses, one `contentError`; after the app rebuilds the editor with a schema widened again, typing works, and the newer client keeps the `figure` |

So widening protects only together with the content check. The check turns "deleted" into "paused", and the rebuild turns "paused" back into "editing". The rebuild costs the person their selection and the editor's undo history. It happens once per new kind, not per edit.

**Limits not tested in the spike.** A node whose parent's content expression is strict, such as a list item that must start with a paragraph, can still fail the schema with a placeholder in the wrong place. The content check then pauses editing instead of deleting. Overlapping marks, whose Yjs keys carry a hash suffix, are stripped to their name, which the spike handles but did not exercise. The placeholder renders no content of an atom node, so text inside an unknown block is kept but not shown.

### A y-prosemirror option or patch

There is no option in y-prosemirror 1.3.7 that keeps unknown content; its sync plugin takes no `onInvalidContent`. y-tiptap 3.2 and later has one: `onInvalidContent`, which TipTap's `Collaboration` sets when the editor has `enableContentCheck`. That option is the patch, already upstream, and it gives N5's behaviour. A patch that projects an unknown element into a generic node would have to change ProseMirror's schema model, which the ProseMirror author rules out (N6). The Yjs maintainer has not planned one either. Widening the schema reaches the same result without a fork.

### A guard in the binding

The NoCentralDB binding collects the editor's Yjs updates before it writes them as changes (RFC 0013, "An editor: the binding"). A guard on the `Y.Doc`'s `afterTransaction` can list each struct the editor's transaction deletes and flag those the editor's schema does not know: an element whose name the schema lacks, a text format whose mark the schema lacks, an attribute the node type does not declare.

| Run | Flagged |
| --- | --- |
| plain y-prosemirror with the older schema | `callout`, `tone`, `highlight` (twice), `chip`, `ref`, `align`: every loss of the scenario |
| the newer schema, ordinary edits: typing, deleting the callout block, removing a highlight | nothing |

Yjs cannot take back a transaction. So the binding refuses to write the flagged update as a change, reloads its `Y.Doc` from the core's state, as RFC 0013 already does for a reader's write, and reports the field as not fully shown. The guard compares against the editor's actual schema. With a widened schema, the placeholders count as known, so a person who deletes a placeholder on purpose is not stopped. The guard does not need TipTap and works with any editor that edits a `Y.Doc`.

### What follows for the binding, whatever D4 decides

1. **Widen the schema before the editor opens.** `@nocentraldb/tiptap` ships the widening as an extension built from the field's document, so the app adds one line.
2. **Require `enableContentCheck`, and rebuild the editor on `contentError`** with a schema widened again. Pin `@tiptap/y-tiptap` 3.2.0 or later and `@tiptap/extension-collaboration` 3.33.0 or later.
3. **Guard every update the editor writes,** and refuse one that deletes a part the editor's schema does not know. This also catches the attribute case (N4) and an app that forgets step 1 or 2.

With these three, every client keeps working on the document. It shows a placeholder for what it cannot interpret and keeps the rest editable. Without them, one older client deletes the newer clients' content on open (N1 to N3). And no code of the SDK, the app or an agent may convert the fragment with a ProseMirror schema outside the editor (N7); the core's tree read must come from Yrs, not from y-prosemirror. That breaks principle 3 and "Never break an existing client or stored data".

The core itself needs none of this. It applies Yjs updates with Yrs and never projects the document into a schema. An agent's edit made in the core, at the Yrs level, deletes only what it addresses.

## 4. Options A, B and C for D4

D4 asks which nodes the core knows. The core needs a vocabulary only where it renders the document itself: as Markdown for `space.get` (D6, accepted), for agents (D5) and in the Rust SDK. Storing, syncing and merging need none. So no option changes what an older editor does with an unknown node: **every option needs the binding of section 3**, because an older client always meets names it does not know. Under A that is a node of a later rule or of the app's own extensions, under B a node of a newer lock, under C a node of a newer app version.

| | A. Fixed vocabulary in the core | B. Vocabulary in the lock | C. The app owns it; the core gives plain text |
| --- | --- | --- | --- |
| **An older client, in the editor** | Needs the binding of section 3. | The same. Its lock names fewer nodes than the newer lock does. | The same. |
| **An older client, in the core's reads** | Renders what rule 1 knows; any other node as its text (RFC 0013's draft). | Renders what its own lock declares; newer nodes as their text. | Plain text and the tree; nothing to miss. |
| **A node an app adds later** | Kept. It reads as its text in Markdown until the core's vocabulary knows it. | The app declares it in the schema, and the lock carries its Markdown form. RFC 0011 refuses a change of a field's `config` for an existing field, so the declaration must be an append-only part of the lock outside the merge rule, like a choice's options (RFC 0005, "Accepted"). That is a new lock rule. | Nothing changes in the core. The app's renderer shows it. |
| **Agents and the Rust SDK** | Markdown with headings, lists, marks, tables, tasks and mentions. An app's custom node is invisible as such. | Markdown including the app's own nodes, in the form the app declared. | Plain text and a tree. An agent sees no headings, lists or marks unless the app renders them for it. An agent without the app, such as a CLI or Rust agent, gets plain text. This undoes D6's Markdown string (Carsten, 2026-11-21). |
| **Schema and lock rules (RFC 0005, RFC 0011)** | None. The vocabulary is the core's. If it is part of rule 1, as RFC 0013's draft says, extending it later is a new rule version, which an existing field reaches only through a migration. | A new lock section with its own additive check: nodes and marks appended, never removed or reformed. Changing a node's Markdown form changes what every agent reads. | None. |
| **What it costs** | A Markdown renderer for a fixed set of names in Yrs, and a parser of the same set for D5. Smallest. | Option A's renderer plus a declarative Markdown form per node, the lock section, its build and device checks, and the SDK's API to declare nodes. Largest. A lock format change comes back to the owner (GENESIS.md). | A plain-text and tree reader. Small in the core; the cost moves to every app and every agent. |

A fourth option came out of the spike:

**D. Option A with a lossless fallback, and the vocabulary outside the merge rule.** The core renders rule 1's vocabulary as Markdown, as A does. Any node or mark outside it is rendered as an inline or block HTML tag with its attributes, which CommonMark allows as raw HTML: `<callout tone="warn">Careful.</callout>`, `<chip ref="obj-42"></chip>`, `<highlight color="green">marked</highlight>`. Nothing is hidden: an agent sees that a callout is there and what it holds, and a quote can stand around it. Undeclared attributes of known nodes, such as `align`, are not shown, and a focused edit keeps them because it changes text, not the element. The vocabulary is a property of the renderer, not of the merge rule: it changes what members read, never the state they converge on. So a later SDK can render a new standard node in its Markdown form without a new rule version. That ties the Markdown to the SDK version, which is what the owner would decide with D.

## Recommendation for D4

**D: option A with a lossless fallback, with the vocabulary outside the merge rule. And, whatever D4 decides, the binding of section 3** (widened schema, content check with rebuild, guard), named in RFC 0013 as a requirement of the binding and as a follow-up ticket.

Why:

- **It meets the owner's goal where the goal can fail.** The goal fails in the editor binding (N1 to N5), not in the core. Section 3 fixes it for every option, and it is the part that keeps older clients editing.
- **It keeps what agents read.** D6 already accepted Markdown, and GENESIS.md's writer persona needs an agent that edits like a co-author. C would give agents plain text. A alone would hide an app's custom nodes as bare text, so an agent could quote across a callout without knowing it is there.
- **It adds no lock rule.** B would make every new node a lock change with its own checks and a lock-format decision, and an older client would still have the older lock. It buys nicer Markdown for custom nodes, which D's tags give in a plainer form.
- **It is the cheapest option that loses nothing.** A's renderer plus one fallback rule.

Not recommended: **B** for its cost and new lock rule with no gain for older clients; **C** because it undoes D6 and leaves agents without structure; **A as drafted** because "a custom node reads as its text" hides the node from agents, and because a vocabulary inside rule 1 can grow only through migrations.

## For D5 (NOC-436)

Sent to the NOC-436 session while this note was written:

- An edit made at the Yjs/Yrs level keeps every unknown part. An agent that edits through a ProseMirror schema, such as a headless TipTap, loses them as an editor does (N1 to N4).
- An anchored edit must change only the quoted range. Inside it, it must keep inline nodes, marks and attributes that its Markdown cannot express. A Markdown round trip of a whole block deletes them. With D's fallback, the tags make them visible to the agent, and the core can refuse an edit that drops a tag the quote contained.
- The guard of section 3 can check an agent's writes as well.
- An agent tool must never build a ProseMirror node of the document with a schema of its own: the conversion alone deletes what that schema lacks (N7). Reading the tree as JSON, or through Yrs, is safe.

## What this note does not answer

- The spike ran in Node with jsdom, not in a browser. It drives the editor through ProseMirror transactions, not keystrokes, and it does not test IME, paste or undo.
- It tests one older and one newer schema. Strict content expressions, nested unknown blocks and overlapping marks are reasoned about, not run.
- It does not run Yrs. That Yrs keeps unknown elements rests on its implementing the same types as Yjs, and on RFC 0013's F9, where Yrs reached Yjs's state byte for byte.
- It does not measure the cost of rebuilding an editor on `contentError` in a large document.
- Whether the Markdown vocabulary may sit outside the merge rule is a question for the owner (option D), since C7 of RFC 0004's plugin contract asks that reads be functions of the state.

## Sources

- Yjs documentation, [Y.XmlElement](https://docs.yjs.dev/api/shared-types/y.xmlelement).
- y-prosemirror 1.3.7, [`src/plugins/sync-plugin.js`](https://github.com/yjs/y-prosemirror/blob/v1.3.7/src/plugins/sync-plugin.js): `createNodeFromYElement`, `createTextNodesFromYText`, `attributesToMarks`, `updateYFragment`. Read in `spike/node_modules/y-prosemirror`.
- y-tiptap 3.3.1, [`packages/y-tiptap` in github.com/ueberdosis/tiptap](https://github.com/ueberdosis/tiptap/tree/main/packages/y-tiptap): `ProsemirrorBinding.ts` (`onInvalidContent`, `_prosemirrorChanged`), `ySyncPlugin.ts` (`filterTransaction`), `createNodeFromYElement.ts`, `deleteYType.ts`. Read in `spike/node_modules/@tiptap/y-tiptap/src`.
- TipTap 3.35.0, `@tiptap/extension-collaboration`, `collaboration.ts`: `enableContentCheck` sets `onInvalidContent` and emits `contentError`.
- TipTap, [Invalid schema handling](https://tiptap.dev/docs/guides/invalid-schema).
- Yjs forum, [Handling unknown nodes/marks in prosemirror schema](https://discuss.yjs.dev/t/handling-unknown-nodes-marks-in-prosemirror-schema/3683), December 2024, with the Yjs maintainer's answers.
- Hocuspocus, [Schema Versioning and Migrations #443](https://github.com/ueberdosis/hocuspocus/discussions/443), November 2022.
- ProseMirror forum, [Adding fallback extensions for unsupported node/mark types](https://discuss.prosemirror.net/t/adding-fallback-extensions-for-unsupported-node-mark-types/4295), December 2021, with the ProseMirror author's answer.
- RFC 0013 in PR #422 (F10, "An editor that cannot show a node", D4 to D6); [RFC 0005, "The additive-change rule"](../../rfc/0005-schema-and-model-lock.md#the-additive-change-rule) and ["Data from a newer model"](../../rfc/0005-schema-and-model-lock.md#data-from-a-newer-model); [RFC 0011, "A type that is not registered"](../../rfc/0011-plugin-field-types.md#a-type-that-is-not-registered).
