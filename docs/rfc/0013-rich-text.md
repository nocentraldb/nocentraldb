# RFC 0013: Rich text as a field type

REQUEST FOR COMMENTS · NOVEMBER 2026 · Ticket NOC-411

This RFC designs `nc.richText`, the field type that [RFC 0005](0005-schema-and-model-lock.md#field-types) reserves and [RFC 0001](0001-nocentraldb.md#field-types-we-ship) promises: a shared document built on Yjs, with Yrs in the core, that people edit in TipTap and agents change with focused edits. It says how a rich-text field's operations travel in a change and fold, how Yjs updates map to those operations, how the TypeScript SDK binds an editor, and what the Rust SDK reads. nexflow.it waits for it: every note body there is rich text.

It checks Yjs's behaviour against the design with four spikes on Yjs 13.6.33, Yrs 0.28.2 and TipTap 3.35 ([What the spikes found](#what-the-spikes-found)), and its findings are the reasons for the choices below. [Follow-up tickets](#follow-up-tickets) lists the work it leads to, in order.

It builds on [RFC 0002](0002-change-and-log-format.md), whose operations carry a rich-text update, and on [RFC 0004](0004-merge-rules-basic-types.md), whose plugin contract every field type meets; [How this RFC relates to the others](#how-this-rfc-relates-to-the-others) names the rest.

**How to give feedback.** Start a discussion in the category [RFC feedback](https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback), naming RFC 0013 and the section you mean, or open a pull request against this file to change its text. [Giving feedback](README.md#giving-feedback) says how, and what happens to your point.

**Status.** Decided, 2026-11-21. Nothing is built yet; [Follow-up tickets](#follow-up-tickets) is the build plan, and [Changes to the other RFCs](#changes-to-the-other-rfcs) lists what the decided RFCs take from it.

**At 0.1.** Not built. Release 0.1 has no rich text: `nc.richText` stays reserved, and nothing of this design is built.

**Out of scope.** Presence and live cursors (Yjs's awareness), which need a channel for messages that are not changes, and a later RFC of their own. Comments and suggestions on a passage. Images, files and other embeds, which need a blob store. Full-text search, which GENESIS.md gives up for local reads. Compaction of a document's history, which belongs to the compaction RFC. Personal rich text ([Erasure](#rfc-0008-and-rfc-0005-erasure)). Plugin field types, which [RFC 0011](0011-plugin-field-types.md) covers; rich text is a built-in type.

## Summary

| Question | Answer |
| --- | --- |
| What is a rich-text field? | One Yjs document per object and field, whose root is the `XmlFragment` named `default`, the shape TipTap's collaboration extension edits. Its type id is `ncdb/richText`, rule 1. |
| How does an edit travel? | As one operation, number 5, "rich-text update": a Yjs update in encoding v1, and for each Yjs client id whose items it adds, the nonce that binds that id to the change's author. One change carries at most one such operation per field. |
| How does it fold? | The core applies the update to the field's Yrs document, in any order of arrival. Yjs keeps content whose dependencies have not arrived as pending, so a gap delays content and never reorders it. |
| What can lose? | Text typed into a block that a concurrent change deleted, and a block attribute, such as a heading's level, set at the same time by two members. Both stay identifiable: the outcome is `lost`, and `space.inspect` gives what was lost and who wrote it. |
| How does TipTap bind? | `space.richText(collection, id, field)` returns a binding with a `Y.Doc`; the app passes it to TipTap's `Collaboration` extension. The SDK collects the editor's updates, writes one change per pause in typing, and applies what other members write. |
| What does an agent do? | It reads the field as Markdown, with a version. It edits by quoting the Markdown it read and giving the Markdown that replaces it; the core looks the quote up in the version the agent read and writes the smallest Yjs update made at that version, which Yjs merges with whatever a person typed since ([An agent: anchored edits](#an-agent-anchored-edits)). |
| What does the Rust SDK read? | `get::<RichText>`, with the document as Markdown, as plain text, as a tree and as the references its mentions hold. |
| What does the relay see? | Nothing new: the update sits inside the sealed payload. It sees one change per pause in typing, not one per keystroke. |
| Does erasure reach rich text? | No, as for every unmarked text. A mention should store only a reference to the person object, never the name, so an erased person's name never was in the text; this is a recommendation, which the core does not enforce. Deleted text stays in the log. |

## Terms

| Term | Meaning in this RFC |
| --- | --- |
| Yjs, Yrs | Yjs is the CRDT library in JavaScript that TipTap's collaboration uses. Yrs is its port to Rust, which reads and writes the same update encoding. |
| Item | One unit of a Yjs document: a run of text, an element, an attribute value. Its id is a client id and a clock. |
| Client id | A number that names one writer of a Yjs document. Its clocks count from 0 without gaps. |
| Update | The bytes that carry items and a delete set from one Yjs document to another. |
| Delete set | The ids of the items an update deletes. |
| State vector | For each client id, the next clock a document expects. |
| Pending | Items a Yjs document holds but cannot place yet, because an item they depend on has not arrived. |
| Document | A rich-text field's Yjs document, one per object and field. |
| Binding | What the TypeScript SDK gives an editor: a `Y.Doc` kept in step with the core's document. |
| Anchored edit | An edit that quotes the Markdown it changes and gives the Markdown that replaces it. |
| Mention | A node in the document that holds a reference to an object, such as a person, and no name. |

## Where things stand today

- [RFC 0001, "Field types we ship"](0001-nocentraldb.md#field-types-we-ship) lists rich text as planned: built on Yjs, with Yrs in the core, edited in TipTap, read by agents as Markdown and changed with anchored edits that turn into minimal operations.
- [RFC 0002, "Operations: object, field and value"](0002-change-and-log-format.md#operations-object-field-and-value) leaves room for it: the value of an operation is a byte string holding a Yrs update. Members fold changes as they arrive, without waiting for gaps ([Dependencies](0002-change-and-log-format.md#dependencies)).
- [RFC 0004, "Operation numbers"](0004-merge-rules-basic-types.md#operation-numbers) reserves the numbers 5 to 63 for later built-in types, rich text among them, and [The plugin contract](0004-merge-rules-basic-types.md#the-plugin-contract) holds every field type, built in or not, to C1 to C7.
- [RFC 0005, "Field types"](0005-schema-and-model-lock.md#field-types) reserves `nc.richText()` and leaves its entry in the lock to this RFC. Its section [Personal data in free text](0005-schema-and-model-lock.md#personal-data-in-free-text) leaves what a mention token looks like to this RFC.
- [RFC 0006, "Where the view lives"](0006-local-view.md#where-the-view-lives) gives a rich-text field no column, since its value cannot be sorted, and keeps its state in `fs_<collection>`.
- [RFC 0009](0009-sdk-api.md#what-this-rfc-does-not-cover) leaves the write and read calls of rich text to this RFC.
- [RFC 0011](0011-plugin-field-types.md) leaves rich text out: it is a built-in type of its own.
- GENESIS.md names the stack: Yjs in the browser, Yrs in the core, TipTap as editor. No crate or package of the repository depends on Yjs or Yrs yet.

## How Yjs works, in the parts that matter here

- **Items and ids.** Every insertion is an item whose id is its writer's client id and a clock. A writer's clocks count from 0 without gaps. Yjs assumes that one client id is one writer: a client id is drawn at random, 32 bits in Yjs, and two writers that draw the same one break the document.
- **Updates commute.** Applying a set of updates in any order gives the same document, and applying an update twice changes nothing. An update whose items depend on items the document does not hold yet is kept as pending and placed once they arrive.
- **Deletions name what their writer saw.** An update's delete set names only items its writer's document held. Two kinds of item are deleted without any delete set naming them: an item inserted into an element that a concurrent writer deleted, and a value written to a map key, such as an element's attribute, that a concurrent write to the same key overrides. Yjs orders such writes by client id, not by time.
- **Garbage collection.** A document drops the content of deleted items and keeps their ids, so its state stays small; a document without it keeps every deleted item.
- **TipTap's mapping.** `@tiptap/y-tiptap`, TipTap 3's fork of y-prosemirror, maps the editor's ProseMirror document to an `XmlFragment`: a node is an `XmlElement` named after its type, with its attributes; text is an `XmlText` whose formatting attributes are its marks. TipTap's `Collaboration` extension binds the fragment named `default` of a `Y.Doc` by default.
- **Undo.** TipTap's undo uses Yjs's `UndoManager`, which undoes only the local writer's own edits, by writing new edits. It needs nothing from the database.

## What the spikes found

F1 to F10 come from a first spike, on Yjs 13.6.33 (the latest stable release; 14.0 is in beta), Yrs 0.28.2 and `@tiptap/y-tiptap` 3.3.1 with `@tiptap/extension-collaboration` 3.35.0; [Appendix: the spike](#appendix-the-spike) shows its code. F11 to F14 come from a second spike, on how an agent and a person edit one document at the same time, with `y-prosemirror` 1.3.7 and `prosemirror-markdown` 1.14.0 ([code and notes](https://github.com/nocentraldb/nocentraldb/tree/carstenkoch/noc-436-research-how-an-agent-and-a-person-edit/docs/research/agent-edits)). F15 to F19 come from a third, on what an older client does with nodes it cannot interpret, with `y-prosemirror` 1.3.7, `@tiptap/y-tiptap` 3.3.1 and TipTap 3.35.0 ([code and notes](https://github.com/nocentraldb/nocentraldb/tree/carstenkoch/noc-437-research-can-every-client-keep-editing-a/docs/research/rich-text-unknown-nodes)). F20 to F24 come from a fourth, the binding in a browser with IME input, paste and undo, in Chromium with the same versions ([code and notes](https://github.com/nocentraldb/nocentraldb/tree/carstenkoch/noc-455-rich-text-1a-spike-the-tiptap-binding-in/docs/research/rich-text-browser-binding)); it also found the echo stopped by the binding's own origin, a pause-based write reaching another tab in about 540 ms and another browser through a local relay in about 600 ms, and the three measures losing nothing with IME input, paste and undo.

| # | Finding | What it means here |
| --- | --- | --- |
| F1 | A one-character insertion is an update of 12 bytes in encoding v1 and 22 in v2. Bold on five characters is 33 bytes, deleting three characters 6 bytes, pasting 5,000 characters 5,010 bytes. | A focused edit stays small, as RFC 0002 expects. Encoding v1 is smaller for small edits, which are the common case. |
| F2 | One member deleted a paragraph while another typed into it. After both synced, the typed text was gone on both, and no update's delete set named it. | This is a loss of an edit whose author had not seen the deletion. Principle 3 needs it identifiable ([Outcomes](#outcomes-applied-replaced-and-lost)). |
| F3 | Two members set a heading's level at the same time. Both ended with the level of the member with the higher client id; the other value was in no delete set. | A block attribute is a last-writer-wins value inside Yjs, decided by client id. Its loser must stay identifiable too. |
| F4 | Updates applied out of order left the later content pending; once the missing update arrived, the document equalled its writer's. | Pending is Yjs's form of RFC 0002's gap. A member folds at once and never waits. |
| F5 | A document rebuilt without one update left a later edit that was placed next to that update's text pending for good. Applying the update and then deleting its items kept the later edit visible. | Un-folding a rejected change cannot simply leave its update out ([Un-folding a rejected change](#un-folding-a-rejected-change)). |
| F6 | Two updates that gave one item id different content left a document whose text depended on the order they arrived in. | A client id must name one writer. Random client ids, or ids another member can claim, can make copies diverge ([Client ids name their author](#client-ids-name-their-author)). |
| F7 | 2,000 insertions and 1,500 deletions came to 42,423 bytes of updates. The document's state was 2,545 bytes with garbage collection, 19,986 without. Its state vector was 4 bytes. | The log grows with every edit; the view's state with the content. Compaction of history is the compaction RFC's. |
| F8 | 60 updates of three members, applied in 20 random orders, gave one text and the same state bytes, in v1 and in v2. | The state has a canonical encoding in this run; the harness has to show it for every generated case (C1). |
| F9 | Yrs applied six updates written by Yjs, in both orders, to the same document as Yjs, and its encoded state was byte for byte Yjs's (136 bytes). | The core folds with Yrs and the editor shows Yjs, and the two agree in this run. A test of their agreement belongs in the harness (C3). |
| F10 | When the editor's schema cannot show a node of the document, y-tiptap deletes that node from the document, unless the editor sets `enableContentCheck`, which makes it report a `contentError` instead. | An editor with an older or narrower schema would delete what it does not know. F15 to F19 measure it and what prevents it. |
| F11 | An agent kept the position of "Friday" from its read; a person then typed "Good news: " before it. The position pointed at `We shi`; a Yjs relative position still pointed at `Friday`. | A plain position goes stale as soon as anyone types before it, so an agent's edit cannot be addressed by one. |
| F12 | A quote looked up in the document as it was at write time was not found once the person had typed a hyphen inside it. Looked up in the version the agent read, and written as a Yjs update made at that version, the edit applied as an 18-byte update and both edits stayed: `We ship on -Monday (tentative).` | Resolving the quote in the read version keeps every keystroke of the person, even inside the quote, and the agent never has to read again because the person typed ([An agent: anchored edits](#an-agent-anchored-edits)). |
| F13 | Two edits of one agent from one read, the second made in a copy that did not hold the first, both used clock 0 of the agent's client id; two copies that received them in different orders ended with different text. | An agent's edits for one field go through one lease, in order, so that its clock continues ([Client ids name their author](#client-ids-name-their-author)). |
| F14 | Making a word bold while a person typed in the same paragraph kept the person's typing; turning the paragraph into a heading lost it, because Yjs cannot rename an element and the binding deletes and re-inserts it. | Changing a block's type is a whole-block rewrite, for an agent and for a person in the editor alike; the person's concurrent typing is `lost` and readable. |
| F15 | Before any editor opened it, the older client's Yjs document held every node, attribute and mark of the newer app. | Yjs and Yrs keep what a client does not know; the core, which only applies updates, keeps it too. |
| F16 | y-prosemirror, y-tiptap without `onInvalidContent` and TipTap without `enableContentCheck` deleted an unknown node from the Yjs document when the editor opened it, before anyone typed, also with the editor read-only, and also with no editor at all, in `yXmlFragmentToProseMirrorRootNode` and `initProseMirrorDoc`. An unknown mark deleted the whole text of its paragraph. `yXmlFragmentToProsemirrorJSON`, which reads without a schema, deleted nothing. | The deletion is an ordinary Yjs edit, so every member loses the content. Nothing may convert the document through a ProseMirror schema outside the editor: the core reads the tree from Yrs. |
| F17 | An attribute the schema does not declare, on a node it knows, raised no error, even with the content check, and the next edit of that node removed it. | The content check alone does not protect attributes. |
| F18 | With `enableContentCheck`, TipTap kept the content, reported one `contentError`, and refused every edit of the field, even in a paragraph the older app knows. It needs `@tiptap/extension-collaboration` 3.33.0 and `@tiptap/y-tiptap` 3.2.0 or later. | The check turns "deleted" into "read-only": better, but the person can no longer work on the field. |
| F19 | With the editor's schema widened from the document before it opened, the content check on and the editor rebuilt on `contentError`, the older client kept editing and lost nothing, also when a new kind of node arrived while it was open. A guard on the binding's transactions flagged every loss of the scenario and nothing in ordinary edits. | Three measures together keep every client editing; the binding requires them ([An editor that cannot show a node](#an-editor-that-cannot-show-a-node)). |
| F20 | A listener registered on the editor's `contentError` event never ran: TipTap's default `onContentError` option threw first, and the editor stayed paused. Given as the editor's `onContentError` option, the rebuild ran. | The rebuild goes in the editor's option. |
| F21 | An IME composition held open past the pause was written before the person committed it, and replaced by a second change when they did. | A pause does not end inside a composition ([An editor: the binding](#an-editor-the-binding)). |
| F22 | A rebuild while a composition was open kept the uncommitted text in the document, lost the committed text, and left the editor without focus. | A rebuild waits for `compositionend` and restores focus and selection. |
| F23 | After a rebuild, undo did nothing with the new editor's own UndoManager; with one UndoManager kept for the `Y.Doc` and passed to each editor, undo took back the text typed before the rebuild. | A rebuild need not cost the undo history. |
| F24 | A guard without the other two measures refused the editor's deletion of unknown parts, reloaded, and the rebuilt editor deleted them again, at every pause; the person's typing, in the same pending write, was refused with it. | The guard reloads once and then makes the field read-only. |

## The field in the schema and the lock

```ts
export const model = nc.model({
  notes: nc.collection({
    title: nc.string(),
    body: nc.richText(),
  }),
});
```

`nc.richText()` builds into the lock as `{ "type": "ncdb/richText", "rule": 1, "since": 4 }`. Rule 1 is this RFC's rule: the operation below, Yjs's integration as Yrs 0.28 implements it, and the outcomes. The Markdown vocabulary of [Reading](#reading) is not part of the rule: it changes what members read, never the state they converge on. A later Yjs format, such as Yjs 14's if it changes how items integrate, is a new rule version, which an existing field reaches only through a migration ([RFC 0004, "How a field type adds operations"](0004-merge-rules-basic-types.md#how-a-field-type-adds-operations)).

- **One document per object and field.** A field's document is its own `Y.Doc`, whose root is the `XmlFragment` named `default`. One document per field keeps each field's state, read and outcomes separate, as for every other type. The name `default` is TipTap's, so an app binds the editor without naming a fragment.
- **Not indexed, not sorted.** A rich-text field cannot be indexed and has no column in `c_<collection>` ([RFC 0006, "Where the view lives"](0006-local-view.md#where-the-view-lives)). A preview in a list reads the field from the object.
- **Not personal in rule 1.** Only basic values can be personal ([RFC 0005, "Personal fields"](0005-schema-and-model-lock.md#personal-fields)); [Erasure](#rfc-0008-and-rfc-0005-erasure) says why rich text stays so.
- **No configuration.** The field takes no options in rule 1. The vocabulary is the core's, not the lock's ([Reading](#reading)).

## How an edit travels in a change

A rich-text edit is one operation of RFC 0002's operation map, with the number 5 from the built-in range of [RFC 0004, "Operation numbers"](0004-merge-rules-basic-types.md#operation-numbers):

```text
operation (RFC 0002's map)
├── 1 object id     16 bytes
├── 2 collection    text
├── 3 field         text
├── 4 op            5: rich-text update
└── 5 value         arguments (CBOR map)
    ├── 1 update    byte string: a Yjs update, encoding v1
    └── 2 clients   map: client id (uint) → nonce (8-byte byte string),
                    one entry for each client id whose items the update adds
```

- **One update per field per change.** The SDK merges the edits of one write into one update with Yjs's own merge, so a change carries one operation per rich-text field it touches. A second operation 5 on the same field in one change is applied too; the rule only keeps changes small.
- **Encoding v1.** It is smaller for the small edits that make up most changes (F1), and it is what Yjs and Yrs exchange by default. Updates in v2 were not taken: they shrink large documents, but every change would pay for the larger header of small ones.
- **Arguments in a map.** Key 5 holds a map, as for RFC 0004's operations, so a later argument can be added the way RFC 0004 adds one: from 4 to 63 if it changes the fold, with a new rule version, and from 64 up if it does not.
- **Size.** A change's payload is padded to a multiple of 256 bytes ([RFC 0002, "Padding"](0002-change-and-log-format.md#padding)), so a change costs at least that, its header and its signature, however small its update. A paste of a long document is a large change and travels the way [RFC 0007](0007-relay.md) carries large changes.

### Client ids name their author

A Yjs client id in NoCentralDB is not drawn at random. It is derived from the member and a nonce:

```text
client id = the low 53 bits, as an unsigned integer, of
            SHA-256("nocentraldb/yjs-client-id" || member id (32 bytes) || nonce (8 bytes))
```

The update's argument 2 lists, for every client id whose items it adds, the nonce. A member that folds the operation recomputes each client id from the change's author and the nonce, and parks the operation as "malformed arguments" when an item's client id is not in the list or does not match ([RFC 0004, "Operations a member does not understand"](0004-merge-rules-basic-types.md#operations-a-member-does-not-understand)). So every item names the member that wrote it, as the change's signature proves, and no member can add items under another member's client id. 53 bits is the largest integer JavaScript holds exactly, and Yjs keeps client ids in JavaScript numbers. A member that searches about 2^53 hashes could find a nonce that gives another member's client id; it gains nothing it cannot do as a writer already, except hiding which of its items are its own, and the rule for the same id twice keeps copies equal.

**The same id twice.** A member restored from a backup can write again under a client id and clocks it used before, as it can reuse an author sequence (RFC 0002). Two changes then give one item id different content, which F6 shows diverging. The fold settles it by the changes themselves: of two changes whose updates give one item id different content, the earlier in the total order of [RFC 0004](0004-merge-rules-basic-types.md#tie-break) keeps the id, and the other's operation is parked as "malformed arguments", on every member. A member that folded the later one first un-folds it when the earlier arrives, as RFC 0006 un-folds a change that a control change rejects. Items with the same id and the same content, such as an update merged twice, are no conflict: Yjs applies them once.

**Reusing a client id.** Each new client id adds an entry to the document's state vector and to the id of every item its writer adds, so a client id per editor session would let a document edited for a year collect thousands. The core therefore keeps, per field, the client ids this member used, the nonce of each and the clock it reached, in a device-local table that is no part of the view, like `confirmed_accounts` ([RFC 0006, "Where the view lives"](0006-local-view.md#where-the-view-lives)). A binding, or a write by an agent, leases one of them for as long as it writes. Two bindings on one device, in two tabs ([RFC 0012](0012-multi-tab-browser-storage.md)), lease two different ids. An id is never leased twice at once, so a member never writes one item id twice, except after a restore, which the rule above settles. An agent's edits for one field go through one lease, in order, so that its clock continues (F13).

**Options not taken.**

- **Random client ids, as Yjs draws them, unchecked.** No argument and no table. But any member could write under another's client id, which F6 shows can make copies diverge, and a random collision does the same. Principle 2 rules it out.
- **One client id per change.** Every id would name its change, and no id would ever be reused. But the state vector and the item ids would grow by one client per change, which for a long-lived document means tens of kilobytes of state vector alone.

## Folding

The core folds an operation 5 with Yrs, natively and in WebAssembly alike, so every member folds by the same code (C3):

1. **Decode.** It decodes the update and checks the client ids. An update Yrs cannot decode, or whose ids do not match, is parked as "malformed arguments".
2. **Apply.** It applies the update to the field's document. Items whose dependencies have not arrived stay pending in the document (F4); they appear once the change that holds their dependencies is folded, whatever the order.
3. **Record.** It records which item ids the operation added and which its delete set named, so that outcomes and un-folding can be computed ([Outcomes](#outcomes-applied-replaced-and-lost)).

**The state.** `fs_<collection>` holds the field's state as one canonical CBOR map: the document as a Yjs update of its whole state, encoded v1 with garbage collection, which holds pending items too; the items each operation added and the deletions it named, as id ranges; and the client ids and their nonces. F8 found the document's bytes equal for every order of arrival; the harness checks it for every generated case. Garbage collection keeps deleted text out of the view's state, but not out of the log ([What erasure reaches](#deleted-text-stays-in-the-log)).

**Performance.** Decoding and encoding the whole document on every apply, as [RFC 0011](0011-plugin-field-types.md#the-field-type-a-plugin-implements) does for a plugin's state, costs time in proportion to the document. The core may keep a document it folds often in memory between applies, as a cache that changes no result. A follow-up spike measures it on large documents.

### Outcomes: applied, replaced and lost

Every operation gets one outcome of [the plugin contract](0004-merge-rules-basic-types.md#what-a-field-type-defines), computed from the changes alone:

- **Lost**, when an item it added is deleted although no delete set of the field names it. That happens exactly in F2 and F3: the item was put into an element a concurrent change deleted, or it set an attribute that a concurrent write overrode. No member writes a delete set for an item it has not seen, so such an item's author lost it to a change that did not see it, which is RFC 0002's concurrent loser ([A late change stays identifiable](0002-change-and-log-format.md#a-late-change-stays-identifiable)).
- **Replaced**, when some item it added is deleted by a delete set, and none is lost. Someone who had seen the text deleted it.
- **Applied**, otherwise: everything it added is in the document, placed or pending.

`$conflicts` names the field while it holds a lost operation, and `space.inspect` lists each lost operation with its author, its stamp and the text it added, read back from the change in the log, since the state no longer holds deleted content. A later write does not clear a lost outcome: as for a basic value, a person who has seen the loss resolves it by writing what they want, and the loss stays readable in the history.

**Pending content.** While items wait for a gap, the read value does not show them and `space.status` reports the gap ([RFC 0002, "Dependencies"](0002-change-and-log-format.md#dependencies)). Their operation's outcome is applied: it is in the state, and it shows once the gap closes.

### Un-folding a rejected change

A change can be rejected after the member has folded it, for example when a removal of its author arrives late and the change lies above the removal's cutoff ([RFC 0008, "What a new change can change"](0008-membership.md#what-a-new-change-can-change)). Its edits must then come out of the document, and the document must read as if the change had never been written.

An example. Carol's laptop, removed from the space while offline, writes " Carol." after "Base." in a note. Bob receives Carol's change before the removal and types " Bob after Carol." right after Carol's last character: Yjs records Bob's text as "placed after Carol's full stop". Then Alice's removal of Carol arrives, and Carol's change is rejected.

Keeping Carol's characters as invisible placeholders keeps Bob's text in place: the note reads "Base. Bob after Carol." Carol's " Carol." is gone, and any text Carol's change had deleted is back.

So the core recomputes the document from the remaining operations plus the rejected change's items, each deleted, and without the rejected change's delete set. The rejected change's text is gone and the text it deleted is back, while the items it added stay as deleted placeholders that later edits can still be placed next to. Text that another member typed inside a block the rejected change added goes with that block; its operation's outcome is `lost`, and `space.inspect` gives the text back. The result is a function of the set of changes, as RFC 0002 requires. The rejected change stays in the log, rejected, as every rejected change does.

**What this does not do.** An attribute the rejected change set, such as a heading's level, does not come back to the value it overwrote: Yjs deleted the older value when the newer one arrived, and the rejected change's value, now deleted too, leaves the attribute unset. The SDK reports the field as un-folded, as it reports every un-folded change ([RFC 0009, "Subscribe"](0009-sdk-api.md#subscribe)).

**Option not taken: leaving the rejected update out**, as every other field type does: recompute without it. It is the simplest rule, but Bob's text would then have nothing to be placed after. Yjs keeps it waiting for Carol's characters, which never come, so Bob's text would disappear for good, although Bob did nothing wrong (F5). The SDK could only report it.

## Reading

A rich-text field's read value is its document. The core renders it on read, from the state, so every member renders the same state the same way (C7):

- **As Markdown**, CommonMark with GitHub's tables, strikethrough and task lists. This is what agents read and what `space.get` and queries return in TypeScript, as a string, so a list of notes shows a preview without another call and `nc.Infer` types the field as `string`.
- **As plain text**, the text of each block, one block per line, for previews.
- **As a tree**, the ProseMirror JSON of the document (`{ type, attrs, content, marks, text }`), as `Data` in Rust and as a plain object in TypeScript, for apps that render the document themselves. The core reads it from Yrs, never through y-prosemirror's conversion, which deletes what its schema does not know (F16).
- **Its mentions**, the references the mention nodes hold, for an app that resolves names.

Storing and syncing a document needs no vocabulary: the core only applies Yjs updates. It needs one only where it renders the document itself, as Markdown. The core knows TipTap's standard vocabulary and renders anything else as an HTML tag, so nothing is hidden from an agent. The vocabulary belongs to the renderer, not to the merge rule, so a later core can render a new standard node in its Markdown form without a migration. Two members with the same core version render the same state the same way (C7); a member on an older core may render a newer standard node as a tag until it upgrades. The Markdown rendering knows the names TipTap's StarterKit and its task lists use:

| Node or mark | Markdown |
| --- | --- |
| `paragraph`, `heading` (`level`), `blockquote`, `bulletList`, `orderedList` (`start`), `listItem`, `taskList`, `taskItem` (`checked`), `codeBlock` (`language`), `horizontalRule`, `hardBreak`, `table`, `tableRow`, `tableHeader`, `tableCell` | Their CommonMark and GitHub forms. |
| `bold`, `italic`, `strike`, `code`, `link` (`href`) | `**`, `*`, `~~`, a backtick, `[text](href)`. |
| `underline`, `highlight` | Their text, unmarked: Markdown has no form for them. The tree keeps them. |
| `mention` (`space`, `id`) | `[@](ncdb:<space id>/<object id>)`, a link without a name ([Mentions](#mentions)). |
| Any other node or mark | An HTML tag with its name and attributes around its content, as a block or inline, such as `<callout tone="warn">Careful.</callout>`. The tree keeps it whole, and the document is never changed because of it. |

The vocabulary needs no configuration, and an app that uses TipTap's defaults needs nothing. The tags show an agent a part outside the vocabulary, so it can quote around it. Whether a client can keep editing parts it cannot interpret does not depend on this vocabulary: an older client always meets names it does not know, and the binding's three measures keep it editing ([An editor that cannot show a node](#an-editor-that-cannot-show-a-node)).

**Options not taken.**

- **The vocabulary declared in the lock,** as the field's configuration, with a Markdown form per node. Custom nodes would render in a form of their own, but every added node would be a change of the field's configuration, which [RFC 0011](0011-plugin-field-types.md#a-type-that-is-not-registered) refuses for a field that exists, and an older client would still hold the older lock.
- **The app owns the vocabulary, and the core renders only plain text.** Nothing in the core would change when an app adds a node. But `space.get` would read plain text, and an agent, or a Rust or command-line tool without the app, would see no headings, lists or marks, and could quote only plain text.
- **A fixed vocabulary that shows any other part as bare text.** Simpler, but it hides a custom node from an agent, which could then quote across a callout without knowing it is there.
- **A read object with `markdown()`, `text()` and `tree()`** in `space.get`. Each form on demand, but a read object would no longer be plain data, unlike every other field.

### Mentions

A mention of a person should store only a reference to the person's object, never the name. It is a recommendation, in this RFC and in the docs, not a rule the core enforces: the core cannot tell a name from other text. The SDK's mention is an inline node named `mention` with two attributes, `space` and `id`, the target's space id and object id as RFC 0009 writes them. It holds no name. An editor shows the name by reading the target from the member's copy when it renders, and the SDK's TipTap extension does that. After an erasure, the person's personal fields are erased and the mention shows a placeholder, and no name was ever stored in the text: this is [RFC 0005's pattern for free text](0005-schema-and-model-lock.md#personal-data-in-free-text), made concrete. TipTap's own Mention extension stores a `label`, the name as typed, in the document; an app that uses it puts the name into the text, where erasure does not reach it, and the SDK's documentation says so.

## Writing

There are three ways to write a rich-text field, and none of them overwrites the document whole (GENESIS.md: "Rich text takes focused edits, never a whole rewrite").

### An editor: the binding

```ts
import { Editor } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import Collaboration from "@tiptap/extension-collaboration";
import { NcMention, NcKeepUnknown } from "@nocentraldb/tiptap";

const body = await notes.richText(model.notes, noteId, "body");

const editor = new Editor({
  enableContentCheck: true,
  onContentError: () => body.rebuild(editor),
  editable: body.writable,
  extensions: [
    StarterKit.configure({ undoRedo: false }),
    Collaboration.configure({
      document: body.doc,
      yUndoOptions: { undoManager: body.undoManager },
    }),
    NcMention.configure({ db }),
    NcKeepUnknown.configure({ document: body.doc }),
  ],
});

// later
await body.close();
```

`space.richText(collection, id, field)` opens a binding: a `Y.Doc` the SDK keeps in step with the field's document in the core, which runs in a worker ([RFC 0009, "Open and close"](0009-sdk-api.md#open-and-close)).

- **On open,** the SDK leases a client id ([Client ids name their author](#client-ids-name-their-author)), creates the `Y.Doc` with it and loads the field's state into it. The object must exist; a binding on an object the copy does not hold is refused with `notFound`.
- **What the editor writes,** the SDK collects. It merges the editor's updates and writes them as one change at each pause in typing: after 500 ms without an edit, after at most 2 s of continuous typing, and when the binding closes or the page is hidden. Each such change is a `space.write` like any other, folded before it is sent. Collaborators see text within about half a second of a pause, and the relay and the log see writing sessions, not keystrokes. If the tab is killed before a pause, up to 2 s of typing is lost, which the SDK's guide says. A pause does not end inside an IME composition: while one is open, the binding holds the write and writes at `compositionend`, or after 2 s if the composition is still open, so collaborators and the log never see characters the person has not committed, and the person never waits longer than for typing.
- **What other members write,** the core folds, and the SDK passes the binding the part of the document it does not hold yet, which Yjs computes from the binding's state vector. It applies that with an origin of its own, so the editor's undo leaves it alone and the SDK does not write it back.
- **A reader** gets a binding with `writable` false, and the editor is made read-only. If an edit reaches the binding anyway, the write is refused with `notAllowed`, and the SDK reloads the `Y.Doc` from the core's state, so the editor never shows text that no change holds.
- **Several tabs** each open their own binding, with their own client id. What one tab writes reaches the other through the shared database's fold ([RFC 0012](0012-multi-tab-browser-storage.md)).

**Options not taken for when the editor writes.** One change per keystroke batch of the editor: collaborators would see every keystroke at once, but each keystroke would cost at least a 256-byte payload, a header and a signature, and the relay would see the typing rhythm. A window each app chooses: more to explain, for a choice most apps never change.

**The package.** `@nocentraldb/tiptap` holds `NcMention`, `NcKeepUnknown` and nothing that TipTap's own collaboration extension already does. The binding itself is in `@nocentraldb/sdk` and works with any editor that edits a `Y.Doc`, though the Markdown rendering and the vocabulary follow TipTap's names.

#### An editor that cannot show a node

Every client must keep editing a document even where it cannot interpret parts of it, such as an older version of the app after a newer one added a node, a mark or an attribute. Yjs keeps those parts (F15). The editor binding is what loses them: by default it deletes an unknown node or mark from the Yjs document as soon as the editor opens it, read-only or not, and the deletion reaches every member (F16). TipTap's content check prevents the deletion but makes the whole field read-only (F18), and does not protect an undeclared attribute (F17).

So the binding requires three measures, which together kept every client editing and lost nothing (F19):

1. **A schema widened from the document.** Before the editor opens, the binding scans the field's document and gives each unknown node, mark and attribute a placeholder type: a node of that name, inline or block as it sits, with its attributes declared without a default value of their own; a mark with its attributes; an extra attribute on a known node. The editor shows a placeholder, such as a box with the node's name, and keeps the rest editable. `@nocentraldb/tiptap` ships this as `NcKeepUnknown`. A person can still delete a placeholder on purpose, as any block.
2. **The content check, with a rebuild.** The binding requires `enableContentCheck: true`, with `@tiptap/extension-collaboration` 3.33.0 and `@tiptap/y-tiptap` 3.2.0 or later. When a kind of node arrives that the open editor does not know, TipTap reports `contentError` and pauses editing instead of deleting; `body.rebuild(editor)` builds the editor again with a schema widened anew. The rebuild goes in the editor's `onContentError` option, not in a listener on the editor's events: TipTap's default for that option throws before any such listener runs, and the editor would stay paused (F20). A rebuild waits while an IME composition is open and runs at `compositionend`, so the person's committed text is never lost (F22). After it, the binding restores the focus and the selection, kept as a Yjs relative position. The binding keeps one UndoManager for its `Y.Doc`, `body.undoManager`, which each editor it builds uses, so a rebuild keeps the undo history (F23).
3. **A guard on what the binding writes.** Before the binding writes an editor's update as a change, it checks which parts the update deletes. An update that deletes an element, a format or an attribute the editor's schema does not know is not written, and the binding reloads once: it loads its `Y.Doc` again from the core's state and reports the field as not fully shown. If the guard refuses again, the binding makes the field read-only: it writes nothing more from that editor, makes the editor read-only and keeps reporting the field as not fully shown, until the app rebuilds the editor with a schema widened anew. It never reloads in a loop, which would refuse the person's typing at every pause (F24). Placeholders count as known, so a deliberate deletion passes. The guard works with any editor that edits a `Y.Doc`.

An editor without `enableContentCheck` is refused: `NcKeepUnknown` throws, as the editor is created, an error that names the missing option, so an app that leaves out the content check finds out at once and not when a newer app writes a node it does not know.

Nothing else in the SDK, an app or an agent's tool may turn the document into ProseMirror nodes with a schema of its own: that conversion deletes what the schema lacks, with no editor (F16). The core never does: it applies updates with Yrs and reads the tree from Yrs.

### An agent: anchored edits

An agent edits the way Claude edits a file: it quotes the text it read and gives the text that replaces it. The quote is looked up in the version of the document the agent read, not in the document as it is when the agent writes, and the edit is written as a Yjs update made at that version, which Yjs then merges with everything written since. A plain position would go stale as soon as anyone typed before it (F11); a quote is a sound address, as long as it is looked up in the version the agent read (F12).

```ts
const body = await notes.readRichText(model.notes, noteId, "body");
// body.markdown: "Intro.\n\nWe ship on **Friday**.\n\nOutro."
// body.version: opaque

await notes.update(model.notes, noteId, {
  body: nc.richText.edit(body.version, [
    { quote: "We ship on **Friday**.", replacement: "We ship on **Monday**, after the review." },
    { after: "Outro.", insert: "A closing line." },
  ]),
});
```

```rust
let body = note_space.read_rich_text("notes", note_id, "body").await?;
note_space.update("notes", note_id, [(
    "body",
    Value::from(RichTextEdit::at(&body.version)
        .replace("We ship on **Friday**.", "We ship on **Monday**, after the review.")
        .insert_after("Outro.", "A closing line.")),
)]).await?;
```

The read gives the Markdown and a version. An edit names the version and one or more quotes with their replacements; the edits of one call go into one change and apply in order. The core, in the member's own copy:

1. takes the document as it was at the version, plus the edits this agent's lease has written since;
2. renders it as Markdown and finds each quote there. A quote must occur exactly once; otherwise the write is refused with `anchorNotFound` or `anchorAmbiguous`, and the agent quotes more. Both depend only on what the agent read, never on what a person typed since;
3. parses the replacement with the same vocabulary and writes the smallest change, text by characters, marks by ranges, blocks by type, under the agent's leased client id ([Client ids name their author](#client-ids-name-their-author)), directly on the Yrs document, so that nodes, marks and attributes the Markdown does not show stay untouched;
4. folds that update into the live document, where Yjs merges it with everything written since the read, as it merges the update of a member that was briefly offline.

A person who types inside the quoted sentence while the agent works keeps every keystroke, and the agent's edit still applies (F12); looked up at write time instead, the same quote would no longer be found and the agent would have to read again. An edit whose quote spans several blocks may also add, remove or reorder whole blocks within the quote, and `{ after: quote, insert: markdown }` or `{ append: markdown }` adds blocks.

**The version.** It is the document's state at the read. The core keeps it, for an agent's open reads, in a device-local cache with a time limit, no part of the view or the log, at about the size of the document's state (2.5 KB for F7's document). An edit that names a version the core no longer holds is refused, and the agent reads again; the error's name comes with the SDK's API review.

**Parts outside the vocabulary.** The HTML tags of [Reading](#reading) show an agent parts outside the vocabulary, so it can quote around them, and the core refuses an edit whose replacement drops a tag its quote contained. Three more rules keep an edit focused: a quote's ends fall on text, not inside markup; a block's type changes only when the replacement changes it, since that rewrites the block (F14); and an agent's edits for one field go through one lease in order (F13).

**What it does not do.** A quote copied from the Markdown is found; one written from memory may not be, since Markdown escapes characters such as `*` and marks can sit inside a word (`**Fri**day`). Two writers who change the same characters at once both keep their edits, which may read oddly. Changing a block's type, such as a paragraph into a heading, rewrites the block in Yjs, so a person's concurrent typing in it is `lost`, as it is when two people do the same in the editor. Errors never contain the quote or any other text of the document, since content never goes into errors ([RFC 0003, "Keys never reach logs, errors or telemetry"](0003-keys-and-cryptography.md#keys-never-reach-logs-errors-or-telemetry)): they name the field and the number of matches.

Anchored edits are core code, so both SDKs have them, and a TypeScript app can use them without an editor.

Prior art points the same way. Tiptap's AI Toolkit edits whole nodes by a hash and answers a document that changed meanwhile with a conflict to retry ([Tiptap AI Toolkit](https://tiptap.dev/docs/ai/ai-toolkit/overview.md)). Liveblocks found that a model editing node ids "could easily lose track of structure, break references, or output malformed operations", and moved to letting the model write text and code compute the change ([Liveblocks](https://liveblocks.io/blog/building-an-ai-copilot-inside-your-tiptap-text-editor)). Claude's own editing tools quote exact text that must occur once ([Claude Code tools](https://code.claude.com/docs/en/tools-reference)). The spike behind F11 to F14, with its notes and sources, is on a [branch](https://github.com/nocentraldb/nocentraldb/tree/carstenkoch/noc-436-research-how-an-agent-and-a-person-edit/docs/research/agent-edits).

**Options not taken.**

- **The quote looked up in the document as it is at write time.** Simpler, with no version and no cache. But the edit is refused whenever the person typed inside the quote, exactly when the two work together, and a person who types steadily can make an agent retry again and again (F12).
- **Plain text within one block, plus calls that insert and delete whole blocks.** Exact and simpler to build, but an agent must translate what it read as Markdown into calls, and cannot change a mark or a block by quoting it.
- **Node ids, with new content for the whole node,** as Tiptap's toolkit does. It needs an id in every block, which the document does not have, and replaces whole nodes, which loses a person's concurrent typing in them (F14).
- **An editing session, in which the agent moves a cursor to a quote and types there,** as Claude's artifacts appear to do. Robust, but many calls and changes per edit; it can be built on anchored edits later, once presence exists.

### The first content

`nc.richText.fromMarkdown(markdown)` writes a whole document, and only into an empty field: on `create`, or on a field that holds no content in the copy. On a field with content it is refused with `richTextNotEmpty`, because replacing a document that others may be editing would overwrite their edits. An app that imports a document writes it once this way and edits it from then on.

### The Rust SDK

The Rust SDK reads a rich-text field with `object.get::<RichText>("body")`: `markdown()`, `plain_text()`, `tree()` as `Data`, and `mentions()` as `Reference`s. It reads a field with its version through `read_rich_text`, edits it with `RichTextEdit::at(version)` and its `replace`, `insert_after` and `append`, and writes a first document with `RichText::from_markdown`. It has no editor binding in rule 1; a native app that wants one asks for it in a follow-up ticket. `space.inspect` gives the lost operations, as in TypeScript.

## What the relay and the other RFCs see

### RFC 0003: encryption

- **The update is content.** It sits in the operation, inside the sealed payload, under the space key ([RFC 0003, "How a member signs and encrypts a change"](0003-keys-and-cryptography.md#how-a-member-signs-and-encrypts-a-change)). The client ids and nonces sit there too; the relay sees none of them.
- **What the relay sees** is what it sees for every change: its size after padding and when it arrives. Writing at each pause in typing shows the relay the rhythm of a writing session, which GENESIS.md already lists among what the relay sees, and not the rhythm of keystrokes, which one change per keystroke would show.
- **No content in logs or errors.** The read value, the state and the operations print `[redacted]` in `Debug`, as every content does, and no error names text of the document.

### RFC 0008 and RFC 0005: erasure

- **Erasure does not reach a rich-text field.** It is unmarked free text, as RFC 0005 says of a rich-text body ([Personal data in free text](0005-schema-and-model-lock.md#personal-data-in-free-text)). A name typed into it stays.
- **Mentions** keep names out of the text, so an app that uses them for persons has nothing in its rich text for an erasure to reach ([Mentions](#mentions)).
- **Option not taken: personal rich text**, `nc.richText().personal({ person })`, with every update sealed under a subkey of the person key. A shred would make the whole document unreadable, history included. But it needs a new RFC 0003 subkey and a change of RFC 0005's rule that only basic values can be personal, and a member without the person key could not fold the field at all. A mention that stores a reference reaches the same goal for names, at no cost to the format.

### Deleted text stays in the log

Deleting text in the editor is not erasing it. The view's state drops deleted content (garbage collection), but the log keeps every update, and so every copy, the relay's ciphertext, archives and backups keep the deleted text under the space key, as they keep every overwritten value. The SDK's guide says so where it explains deletion, and the FAQ says it next to erasure.

## How rich text meets the plugin contract

| # | How `ncdb/richText` meets it | How the harness checks it |
| --- | --- | --- |
| C1 | Yjs updates commute (F4, F8); the fold adds only rules that read the changes alone: client ids, the same id twice, outcomes and un-folding. | Generators for several members that type, format, split and join blocks, delete blocks, set attributes, and sync now and then; three copies in random orders compare their state bytes. |
| C2 | Yjs applies an update it holds as a no-op; the bookkeeping is sets of ids. | Duplicates in every delivery. |
| C3 | The core folds with Yrs only, natively and as wasm32. The editor's Yjs is a copy of what the core folded, not a second fold. | The same seed natively and as wasm32. Beside it, a set of recorded updates whose expected document Yjs and Yrs must both reach, because the editor shows Yjs's result (F9). |
| C4 | Applied, replaced and lost as defined above; lost content is readable through `space.inspect`. | The generator knows what each member had seen, so it checks that `lost` names exactly the items deleted by a change that had not seen them. |
| C5 | An update Yrs cannot decode, or with ids that do not match, is parked. | Arbitrary bytes and truncated, duplicated and forged updates mixed into the generated sets. Yrs must not panic on them, natively or in wasm, which a fuzzing ticket checks first. |
| C6 | Broken fixtures: a fold that draws client ids at random, one that accepts any client id, and one that un-folds by leaving the update out. Each must fail C1 or C4. | Runs each and expects a failure. |
| C7 | Markdown, plain text, tree and mentions are functions of the state. | Compares all four reads, and the outcomes, across copies. |

The versions of Yrs and Yjs are pinned exactly, and an upgrade of either has to pass the recorded updates unchanged. One that does not changes the rule, which only a new rule version may do.

## What changes for developers

- `nc.richText()` builds, and a rich-text field reads as Markdown in `space.get` and queries.
- `space.richText(collection, id, field)` opens a binding for an editor; `@nocentraldb/tiptap` gives `NcMention`.
- `readRichText(...)` reads a field's Markdown with its version; `nc.richText.edit(version, [...])` edits it by quotes; `nc.richText.fromMarkdown(...)` writes a first document. The Rust SDK has the same calls.
- New errors: `anchorNotFound`, `anchorAmbiguous`, `richTextNotEmpty`, and one for a version the core no longer holds, named in the API review.
- `space.inspect` lists a rich-text field's lost operations, and `$conflicts` names the field while it has any.

## Changes to the other RFCs

Step 7c of the build plan writes these into the decided texts ([How a decided RFC changes](README.md#how-a-decided-rfc-changes)):

- **RFC 0001**, "Field types we ship": rich text is designed in this RFC.
- **RFC 0002**, "What this RFC does not cover" and "Operations": rich text's value is this RFC's operation 5.
- **RFC 0004**, "Operation numbers": number 5 is "rich-text update", for `ncdb/richText`.
- **RFC 0005**, "Field types": `nc.richText()` maps to `ncdb/richText` rule 1; "Personal data in free text" points to this RFC's mentions; "Public names" gains the new names.
- **RFC 0006**, "Where the view lives": the device-local table of client ids.
- **RFC 0009**: the binding, the read with a version and the anchored edits, the errors and the public names.

## What this does not do

- **No presence or cursors.** Cursors need messages that are not changes: not signed into the log, not kept, encrypted for the space, and relayed at once. That is a relay feature of its own, with metadata of its own, for a later RFC; designing it here would hold back the rest.
- **No erasure of rich text**, and deleted text stays in the log ([What erasure reaches](#deleted-text-stays-in-the-log)).
- **No protection against a writer.** A member that may write can delete a whole document, as it can clear any field; the deletion is a change in the log, and an app can bring the text back from history.
- **Typing in the last 2 s can be lost** if the tab dies before the pause ([An editor: the binding](#an-editor-the-binding)).
- **An app that leaves out the binding's measures**, or converts the document with a ProseMirror schema of its own, deletes what that schema does not know; the guard catches it in the binding, not in an app's own code ([An editor that cannot show a node](#an-editor-that-cannot-show-a-node)).
- **Custom nodes read as HTML tags**, not in a Markdown form of their own ([Reading](#reading)). An attribute the vocabulary does not show, on a node it knows, is kept but not shown.
- **A rejected change's attribute value** leaves the attribute unset, not restored ([Un-folding a rejected change](#un-folding-a-rejected-change)).
- **No embeds**, such as images or files, and no comments or suggestions.
- **No Yjs 14.** Rule 1 is Yjs 13's integration; a later Yjs whose integration differs is a new rule.

## Follow-up tickets

The build plan, in order. Steps on one line of the table may run in parallel; a step starts when the steps it needs are merged.

| Step | Ticket | Needs | In parallel with |
| --- | --- | --- | --- |
| 1 | Spikes 1a and 1b | nothing | each other |
| 2 | Core rule (2a), schema and lock (2b) | 1b for 2a | each other |
| 3 | Core un-folding (3a), core reading (3b) | 2a | each other, and with 4 |
| 4 | TypeScript binding and `@nocentraldb/tiptap` | 1a, 2a, 2b | 3 |
| 5 | Agent edits in the core | 3b | 6 |
| 6 | Rust SDK reads | 3b | 5 |
| 7 | Agent edits in both SDKs, docs, RFC texts | 4, 5, 6 | each other |

1. **Spikes, before any build ticket.**
   - **1a. The TipTap binding in a browser.** How the binding avoids echoing its own writes; the latency of a pause-based write between two tabs and two browsers; and the three measures with IME, paste and undo, which F19 did not run.
   - **1b. Yrs under bad input and large documents.** Fuzz decoding and applying of arbitrary updates for panics, natively and in wasm32; measure apply with and without a cached document on a 100 KB document.
2. **The core rule and the schema.**
   - **2a. Core: the field type `ncdb/richText` rule 1.** State, operation 5, client ids and their check and lease, the same id twice, outcomes, generators and broken fixtures for C1 to C7, and the Yjs and Yrs agreement vectors.
   - **2b. Schema and lock: `nc.richText()`** in `ncdb model build` and the check on the device.
3. **The core's un-folding and reads.**
   - **3a. Core: un-folding rich text**: placeholders for the rejected change's items, its deletions undone.
   - **3b. Core: reading.** Markdown in the core's vocabulary, with HTML tags for any other part; plain text; the tree, read from Yrs, never through y-prosemirror's conversion; and mentions.
4. **TypeScript: the binding and `@nocentraldb/tiptap`**, through the bridge to the core: the lease of client ids, the write at each pause, the reload of a refused write, `NcMention`, and the three measures, each with a test that fails without it: an older schema opening a newer document loses nothing and stays editable (widening); a node kind arriving while the editor is open pauses and resumes after the rebuild (content check); an update that deletes an unknown element, format or attribute is not written, a second refusal makes the field read-only, and a deliberate deletion of a placeholder is written (guard); an editor without `enableContentCheck` is refused; a composition is not written before `compositionend`, and a rebuild waits for it and keeps the undo history.
5. **Core: anchored edits**: the read version and its cache, the lookup, the Yrs-level diff, `anchorNotFound` and `anchorAmbiguous`. With three rules: a quote's ends fall on text, not inside markup; a block's type changes only when the replacement changes it; and an agent's edits for one field go through one lease in order.
6. **Rust SDK: `RichText` reads**: Markdown, plain text, tree and mentions, and `RichText::from_markdown`.
7. **The rest.**
   - **7a. Agent edits in both SDKs:** `readRichText` and `nc.richText.edit` in TypeScript, `read_rich_text` and `RichTextEdit` in Rust, and the error for an expired version.
   - **7b. Docs:** a rich-text guide, with the three measures, mentions, and the FAQ on deleted text and erasure.
   - **7c. RFC texts:** the changes to RFC 0001, 0002, 0004, 0005, 0006 and 0009 listed above.

Later RFCs, not build tickets: presence and cursors, personal rich text if it is ever wanted, and the compaction of a document's history.

## How it is tested

No code changes with this RFC. The spikes back its findings. The design is tested where it is built: the harness checks C1 to C7 for `ncdb/richText` (step 2a), each SDK's binding and calls have their own tests, and the agreement vectors pin Yjs and Yrs together.

## Agreement with GENESIS.md

| [GENESIS.md](../../factory/GENESIS.md#principles-in-priority-order) says | This RFC |
| --- | --- |
| Principle 1, "The relay never reads content." | Updates, client ids and nonces sit inside the sealed payload. Writing at pauses shows the relay sessions, not keystrokes. |
| Principle 2, "Every copy converges." | Yrs folds everywhere; client ids are bound to their author; the same id twice, outcomes and un-folding are settled by the changes alone. |
| Principle 3, "Nothing is lost silently." | Text lost to a concurrent deletion and attributes lost to a concurrent write are `lost` and readable; an editor that cannot show a node does not delete it. |
| Principle 4, "Say plainly what it does not do." | [What this does not do](#what-this-does-not-do), and erasure stated plainly. |
| Principle 6, "The apps only read from a result that is computed." | The binding's `Y.Doc` is loaded from the core's folded state and fed by its folds. |
| Principle 7, "One core lets you build apps for almost any environment." | The fold and the reads are core code, the same natively and in wasm. Only the binding is TypeScript, since the editor is. |
| "Never overwrite more than you change. Rich text takes focused edits, never a whole rewrite." | Edits are Yjs updates; an agent's anchored edit writes the smallest change, made at the version it read; a whole document is written only into an empty field. |
| "Never write content … into logs." | Reads and states print `[redacted]`, and no error names text of the document. |

## Appendix: the spike

The spike ran in Node.js 24 with `yjs@13.6.33`, `@tiptap/y-tiptap@3.3.1` and `@tiptap/extension-collaboration@3.35.0`, and in Rust with `yrs = "=0.28.2"`. Two Yjs documents with fixed client ids stood for two members; an update is what a document emits for a local transaction.

```js
// F2: one member deletes a paragraph while another types into it.
a.getXmlFragment("body").delete(1, 1);
b.getXmlFragment("body").get(1).get(0).insert(7, " Bob typed this.");
sync(a, b); sync(b, a);
// a and b: <paragraph>First.</paragraph>
// A's delete set names only client 1; B's update deletes nothing;
// the merged document's delete set names client 2's 16 characters.

// F5: rebuild without Carol's update.
Y.applyUpdate(c, base); Y.applyUpdate(c, bobAfterCarol);
// <paragraph>Base.</paragraph>, pending: missing [1, 7]
// With Carol's update applied and her 7 characters deleted instead:
// <paragraph>Base. Bob after Carol.</paragraph>

// F6: two documents with client id 4 insert "P" and "Q" at clock 1.
// applied P then Q: <paragraph>XP</paragraph>; Q then P: <paragraph>XQ</paragraph>
```

F8 applied 60 updates of three documents, each editing at random and syncing now and then, in 20 random orders to fresh documents, and compared `encodeStateAsUpdate` in v1 and v2 and `mergeUpdates`. F9 wrote six updates (two paragraphs and a heading, bold, an insertion and two concurrent heading levels) from Yjs to files, applied them in Rust with `Update::decode_v1` and `apply_update` in both orders, and compared the XML string and `encode_state_as_update_v1` with Yjs's. F10 read `createNodeFromYElement` and `createTextNodesFromYText` in `@tiptap/y-tiptap`: without `reportInvalidContent` they call `deleteYType` on content the schema rejects, and TipTap's `Collaboration` extension passes `onInvalidContent` only when the editor sets `enableContentCheck`.

## How this RFC relates to the others

It builds on [RFC 0002](0002-change-and-log-format.md), whose operation map carries the rich-text update and whose fold never waits for a gap; on [RFC 0004](0004-merge-rules-basic-types.md), whose operation numbers and plugin contract C1 to C7 the type meets; on [RFC 0005](0005-schema-and-model-lock.md), which reserves `nc.richText` and leaves the mention token to this RFC; on [RFC 0006](0006-local-view.md), which keeps the field's state in `fs_<collection>`; on [RFC 0003](0003-keys-and-cryptography.md), whose space key seals the update; on [RFC 0008](0008-membership.md), whose un-folding this RFC applies to a document; on [RFC 0009](0009-sdk-api.md), whose write, read and inspect calls it extends; and on [RFC 0012](0012-multi-tab-browser-storage.md), whose tabs each open their own binding. [RFC 0011](0011-plugin-field-types.md) leaves rich text out: it is a built-in type.

## Decisions

- **2026-11-22 · The binding holds a write while an IME composition is open, until `compositionend` and for at most 2 s.** ([decision](../../factory/decisions/2026-11-22-NOC-467-the-rich-text-binding-holds-a-write-during-an-ime-composition-for-at-most-2-s.md))
- **2026-11-22 · A rebuild waits for `compositionend`, then restores the focus and the selection.** ([decision](../../factory/decisions/2026-11-22-NOC-467-a-rich-text-editor-rebuild-waits-for-compositionend-and-restores-focus-and-selection.md))
- **2026-11-22 · One UndoManager per binding outlives the rebuilds, so a rebuild keeps the undo history.** ([decision](../../factory/decisions/2026-11-22-NOC-467-the-rich-text-binding-keeps-one-undomanager-across-editor-rebuilds.md))
- **2026-11-22 · The guard reloads once, then makes the field read-only; an editor without `enableContentCheck` is refused.** ([decision](../../factory/decisions/2026-11-22-NOC-467-the-rich-text-guard-reloads-once-then-the-field-is-read-only-and-an-editor-without-the-content-check-is-refused.md))

## History

- 2026-11-22 · First published (NOC-411, #422)
- 2026-11-22 · The binding's example rebuilds from `onContentError`; a composition holds the write and a rebuild; one UndoManager outlives rebuilds; the guard reloads once, as the owner decided after the browser spike (NOC-455, NOC-467, #454)
