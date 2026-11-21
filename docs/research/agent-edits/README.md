# How an agent and a person edit one rich-text document at the same time

RESEARCH NOTE · NOVEMBER 2026 · Ticket NOC-436 · Feeds [RFC 0013](../../rfc/README.md)'s D5 (PR #422)

The owner took D5 out of the RFC 0013 decision. D5 is how an agent edits rich text. The draft says: the agent quotes Markdown and gives the Markdown that replaces it. The owner doubts that this is reliable and stable. This note answers the owner's four questions, measures what happens while a person keeps typing, and ends with options and a recommendation. It changes no RFC and no product code. The owner decides.

GENESIS sets the bar: "The agent edits like a careful co-author: it makes focused changes instead of overwriting a whole paragraph or the complete document", and "Rich text takes focused edits, never a whole rewrite."

## The short answer

| Question | Answer |
| --- | --- |
| How does Claude edit an artifact? | With an exact find and replace: it names an `old_str` and a `new_str`. What looks like typing is the screen showing the replacement while it streams in. No cursor moves through a shared document [5][6]. |
| How does Claude Code edit files? | The same way: an exact `old_string` that must occur exactly once, a `new_string`, no fuzzy matching, and a read of the file first [4]. |
| Can ProseMirror be edited as precisely? | Yes. Steps such as `ReplaceStep` and `AddMarkStep` change exact positions [9]. But a position is an integer that moves when anyone types before it, and in a Yjs document the steps are not what travels: the binding diffs the document into Yjs operations [12]. |
| How do an agent and a person edit at once without getting in each other's way? | The agent's edit must be small, and it must be made against the version the agent read, then merged by Yjs. The spike shows that this keeps every keystroke of the person, even inside the text the agent changes, and never sends the agent back to re-read because the person typed (S3). |
| What should D5 be? | Keep Markdown quoted and Markdown replaced, but resolve the quote against the version the agent read, not against the document at write time, and write the edit as a Yjs update made at that version. Option C below. |

## The owner's questions

### Claude's artifacts

A claude.ai artifact is edited with an `update` command that takes the artifact's id, an `old_str` and a `new_str`. Bram Cohen describes it from the outside: the call names text "which appears somewhere in the file and needs to be removed" and the text it "should be replaced with" [6]. Anthropic's own text editor tool for the API works the same way, with a `str_replace` command whose `old_str` "must match exactly, including whitespace and indentation" [5].

What the owner saw, Claude placing a cursor and typing, is how the client shows the call while its arguments stream in. Cohen writes that the old text "is removed on screen in real time" as `old_str` arrives [6]. Anthropic does not document this, so it is an observation, not a specification. There is no cursor in a shared document and no other writer: the artifact belongs to one conversation. The feeling of watching someone type is presentation. Section [How the agent's edit shows](#how-the-agents-edit-shows) uses that.

### Claude Code

Claude Code's Edit tool "performs exact string replacement". It "doesn't use regex or fuzzy matching", `old_string` "must appear exactly once", and when it appears more than once Claude quotes a longer string or sets `replace_all` [4]. Claude reads the file before it edits. A file that changed on disk since the read can still be edited when `old_string` still matches the current content exactly and once; otherwise Claude reads again [4].

So Claude's own tools quote exact text and resolve the quote against the file as it is now. That works because a file on disk rarely changes while Claude edits it. A shared document does change: that is the case this note is about.

Other tools made other choices. Aider, a coding agent, found that GPT-4 Turbo made far fewer mistakes with unified diffs than with search and replace blocks, and that turning off its flexible matching of an edit multiplied editing errors by nine [8]. Aider lets each model use the format it handles best [7]. It also found that models are "terrible at working with source code line numbers" [8]. Those are GPT-4 results from 2023, not Claude's, but they point the same way: models are good at quoting text, bad at counting positions.

### ProseMirror

A ProseMirror document is addressed by integer positions: every character is one token, and entering or leaving a node is one token [9]. A transaction is a list of steps, such as `ReplaceStep` or `AddMarkStep`, each with a map from old positions to new ones; a step can be inverted and mapped through other steps, which is how ProseMirror's own collaboration rebases a client's steps over a server's [9][10]. Rebasing can fail: "there is no guarantee that it can still be validly applied", and then the step is dropped [9].

So yes, a ProseMirror document can be edited exactly. Two things limit what that buys an agent:

- **A position goes stale.** "If you insert a character, all positions after that character now point one token before their old position" [9]. In the spike, a position the agent took when it read pointed at `We shi` instead of `Friday` once the person had typed "Good news: " before it (S1).
- **In a Yjs document the steps do not travel.** TipTap's collaboration uses y-prosemirror (forked as `@tiptap/y-tiptap`). Its sync plugin takes the whole new ProseMirror document after a transaction and compares it with the Yjs document: an element whose name matches is updated in place, attributes are set one by one, and the text of a run is changed by a prefix and suffix diff [12]. What reaches the other members is Yjs items, not ProseMirror steps. The precise unit of a Yjs document is the item id, and its stable position is the Yjs relative position [11].

### Simultaneous editing

Yjs is a CRDT: updates commute, and every copy that has the same updates has the same document (RFC 0013's F4 and F8). So the question is not whether the person's and the agent's edits merge. They always do. It is what each edit says, and against which version it was computed. The spike answers that in [What happens while the person keeps typing](#what-happens-while-the-person-keeps-typing).

## Four ways to say where an edit goes

| Way | What the agent names | When the text moved under it | Who uses it |
| --- | --- | --- | --- |
| Positions | An integer, such as ProseMirror's, or Google Docs' UTF-16 index | Points at the wrong text (S1), unless it is mapped through every change since the read | ProseMirror steps [9]; the Google Docs API [14] |
| Yjs relative positions | An item id and a side (`assoc`) | Still points at the same character, which every copy agrees on; resolves to nothing when that content is gone [11][12] | y-prosemirror's selection and cursors [12]; Yrs as `StickyIndex` [13] |
| Quoted text | The exact text it changes | Found again if it is unchanged; refused if anyone typed inside it (S2), unless it is looked up in the version the agent read (S3) | Claude's artifacts and Claude Code [4][6]; the draft D5 |
| Node ids | An id or hash per block, and new content for the whole block | Works while the block exists; the edit replaces the whole block | Tiptap's AI Toolkit, `_hash` with `replace`, `insertBefore`, `insertAfter` [15]; BlockNote AI, block ids with add, update and delete [16] |

Two products add to this:

- **Tiptap's AI Toolkit** has the model read JSON or a compact "Shorthand" and edit whole nodes by their `_hash` [15]. When the cloud document changed while a tool ran, its execute call returns `409 concurrent_edit_conflict`: "Retry the request with the latest document state" [15]. Its streaming endpoint applies the edit to the live collaborative document "token by token", so collaborators watch it, and it can turn the edit into tracked-change suggestions that people accept or reject [15].
- **Liveblocks**, building a copilot inside a Tiptap editor, first had the model emit operations on node ids: "this proved unstable — the model could easily lose track of structure, break references, or output malformed operations." They switched to having the model rewrite the document and diffing the result: "LLMs prefer rewriting, not patching" [17]. That rewrite is the opposite of GENESIS's focused edits, but the diff that follows is the same idea as the draft D5: let the model write text, and let code work out the smallest change.

Google Docs is the closest relative to this note's recommendation. Its API addresses edits by index, and a write may name the revision it was computed against: with `targetRevisionId`, "the changes produced by this write request are applied against the collaborator changes" made since [14]. The server does the merging, and the client never re-reads because someone typed.

## What happens while the person keeps typing

The spike ([`spike/`](spike/)) ran in Node.js 24 with `yjs@13.6.33`, `y-prosemirror@1.3.7` and `prosemirror-markdown@1.14.0`. Two Yjs documents stand for a person (client id 1) and an agent (client id 2). The agent edits as the editor binding does: it builds a new ProseMirror document with steps and lets y-prosemirror's `updateYFragment` write the difference into Yjs. The document starts as three paragraphs: "Intro.", "We ship on Friday." and "Outro.". `spike/output.txt` holds the printed results; `node spike.mjs` repeats them.

| # | What happened | Result |
| --- | --- | --- |
| S1 | The agent kept the position of "Friday" from its read; the person typed "Good news: " before it. | The plain position now points at `We shi`. The Yjs relative position still points at `Friday`. |
| S2 | The agent's quote "Friday" was looked up in the document as it is when the agent writes. | When the person had typed in another paragraph, the edit applied. When the person had typed a hyphen inside "Friday", the quote was gone and the edit was refused: the agent would have to read again. |
| S3 | The same, but the quote was looked up in the version the agent read, the edit made in a copy at that version, and the result merged as a Yjs update. | The edit applied, as an 18-byte update. Both edits stayed: `We ship on -Monday (tentative).` The agent's "Fri" became "Mon"; the person's hyphen and " (tentative)" stayed theirs. |
| S4 | Neither had seen the other's edit when it wrote: the person typed " Bob typed this." at the end of the paragraph, the agent changed "Friday" to "Monday". | A focused edit kept both: `We ship on Monday. Bob typed this.` Replacing the whole paragraph lost the person's sentence. Replacing the paragraph's whole text kept it, but in the wrong place: ` Bob typed this.We ship on Monday.` |
| S5 | The agent typed a sentence word by word at the end of a paragraph while the person typed at the same place, each keeping its place as a relative position. | Each writer's words stayed together: `We ship on Friday. I type here too. The agent adds this.` |
| S6 | The agent made two edits from one read. | When the second copy held the agent's first edit, the second continued the agent's clock (client 2, clock 3) and all was well. When it did not, both edits used clock 0 of client 2, and two copies that received them in different orders ended with `We ship on Monevisedday.` and `We ship on day.`: RFC 0013's F6. |
| S7 | A document with `5 * 3`, a bold "Fri" inside "Friday", and two equal paragraphs, rendered as Markdown. | Markdown shows `5 \* 3`, `\[draft\]` and `**Fri**day`. An agent that quotes "5 * 3" or "Friday" from memory instead of copying finds nothing. The plain text holds "Friday". "Ship it." matches twice. |
| S8 | Sizes. | Typing " afternoon" was a 21-byte update; "Friday afternoon" to "Monday morning" 33 bytes. |
| S9 | The agent made "Friday" bold, then, in a fresh run, made the paragraph a heading, each time while the person typed in the same paragraph. | Bold kept the person's sentence: a mark is an attribute on a range of text. The heading lost it: Yjs cannot rename an element, so y-prosemirror deletes the paragraph and inserts a heading, and the person's concurrent typing went with the old paragraph. |

What follows from it:

- **Focused edits matter more than anything else.** S4 and S9 show that the cost of a broad edit is the person's typing. A diff that turns the agent's intent into the smallest change, as the draft D5 asks, is the right core.
- **Resolve against the version the agent read.** S2 shows that looking the quote up again at write time fails exactly when the person works on the same sentence, which is when an agent and a person work together. The draft's promise that "a person typing inside the quoted sentence keeps what the edit did not touch" holds for typing that arrives after the agent wrote (S4), not for typing the agent's copy already holds when it writes: that typing makes the quote fail (S2). A person who types steadily can make an agent retry again and again. S3 shows that the edit made against the read version applies and merges, as a Yjs client that was briefly offline would. Google Docs' `targetRevisionId` does the same [14].
- **The same characters, changed by both, merge but may read oddly.** In S3 the person's hyphen landed before "Monday". Nothing was lost and the person sees it, but it is not what either intended. No approach can do better without asking one of them.
- **Changing a block's type is a whole-block rewrite in Yjs** (S9). That holds for every approach on a Yjs document, the editor's own included: a person who turns a paragraph into a heading while another person types in it loses that typing too. An agent should change a block's type only when it means to, and the outcome is `lost` under RFC 0013's outcomes, so it stays identifiable.
- **The agent's fork must continue the agent's clock** (S6). This ties D5 to D1, below.

## Markdown as the quote

The owner's doubt is fair, and S7 shows where it bites:

- **Escapes.** Markdown escapes characters that mean something in it. An agent that copies what it read quotes `5 \* 3` and is found. An agent that quotes from the user's words or from memory writes `5 * 3` and is not.
- **Marks inside words.** `**Fri**day` does not contain "Friday". An agent that wants to change the word has to quote the markup.
- **Marks Markdown cannot show.** Underline and highlight render as plain text (RFC 0013's D4). The agent cannot see them, and a replacement cannot say whether they stay. The diff keeps the marks of characters it does not change, so they survive an edit around them, but an agent cannot add or remove them.
- **What the Markdown cannot say must survive the edit.** The parallel research for D4 (NOC-437, `docs/research/rich-text-unknown-nodes/`) found that Yjs keeps every element, attribute and format it does not know, but whatever passes through a ProseMirror schema loses it: an unknown node is deleted, an unknown mark takes its paragraph's whole text with it, and an undeclared attribute is removed when the node is written again. A Markdown round trip has the same risk. So the edit must change only the quoted range, and inside it keep the inline nodes, marks and attributes that the Markdown did not show. The core should diff at the Yrs level, not by rebuilding nodes from a schema. This spike's ProseMirror path stands in for that diff only because it is shorter to write in Node.
- **Where a quote ends.** A quote that starts or ends inside markup, such as `on **Fri`, has no clean place in the document. The core needs a rule: map Markdown offsets to document positions while rendering, and refuse a quote whose ends fall inside markup.

And where Markdown is strong:

- It is what the agent reads, so "quote what you read" holds. Claude's tools work on exactly that rule [4][5], and Claude copies exact text well.
- One string can say text, marks and blocks at once. Plain text cannot ask for bold, and a set of separate calls is what Liveblocks found models handle badly [17].
- The draft's matching rules already answer ambiguity: a quote must occur once, and the agent quotes more (S7's "Ship it.").

So Markdown is a sound language for the quote, if the quote is checked against what the agent read, its ends must fall on text, and the core maps it back exactly. What makes the draft fragile is not Markdown, it is the moment of lookup: S2.

## How the agent's edit shows

- **Who wrote it.** Under D1 every Yjs item carries a client id derived from its member. The agent's characters are therefore its own, in every copy, without a mark in the document: in S3 and S5 the spike reads, per run of text, which client wrote it. An editor can show the agent's text in a colour: y-prosemirror's snapshot view already marks text as added or removed by a user, found from the item's client id through Yjs's `PermanentUserData` [12]. That costs nothing in the document and needs no new node or mark.
- **A cursor and a name.** A live cursor is presence: Yjs keeps it in "awareness", outside the document, and "Awareness information isn't stored in the Yjs document" [12]. TipTap's `CollaborationCaret` draws the cursors of every client with a name and a colour [12]. An agent that sends presence shows like a person. RFC 0013's D8 leaves presence to a later RFC with an encrypted channel for messages that are not changes, so it is not in rule 1.
- **Typing.** Tiptap's streaming writes the agent's edit into the live document piece by piece and lets the editor fade in the new text with a decoration, `AiInsertReveal`, which "adds no marks and never touches the document" [15]. BlockNote shows an "agent cursor" with a name and colour while the AI writes, and lets the user accept or reject [16]. In NoCentralDB the effect can be had in two ways: an editor animates text that arrives from an agent, which needs nothing from the core; or the agent streams its edit as several small changes (S5 shows the place stays right). The first gives the look of Claude's artifacts without a change per word.
- **Suggestions to accept or reject.** Tiptap and BlockNote both offer them [15][16]. They are a document-level feature of their own (a suggestion has to live somewhere every member sees), and out of D5's scope.

## How this fits D1 and D3

- **D1, client ids.** An edit made in a copy at the read version is written under the agent's leased client id. Two rules follow from S6. The copy must hold the agent's own edits made since the read, so the clock continues; otherwise two edits share an item id and copies diverge. And the copy that folds must not itself be a writer under that id: Yjs gives a document a new client id when an update brings items under its own id that it did not write (the spike printed "Changed the client-id because another client seems to be using it" until it kept them apart [3]). The lease the draft already describes covers both, if an agent's edits for one field are written in order through one lease.
- **D3, when the editor writes.** D3 batches a person's keystrokes into one change per pause. An agent's edit is not typed, so each edit call is one change, folded before it is sent, as any `space.write`. A streamed edit would fall under D3's window: about one change per half second to two seconds, each with its own header and signature. That is why animating on the reader's side is cheaper than streaming.

## How this fits D4

The PR's D4 now has three options, and the owner decides D4 together with D5. Option C below needs the core to render Markdown, so it goes with D4's A (a fixed vocabulary in the core) or B (the vocabulary in the lock). With D4's C, where the app owns the vocabulary and the core renders only plain text, an agent can quote only plain text: it sees no headings, lists or marks, and cannot change them by quoting. The lookup in the read version and the merge by Yjs do not depend on the quote's language, though. They work for plain-text quotes too, so the main finding of this note holds whichever D4 the owner picks.

## What an agent API could look like

The read returns the Markdown and a version. The edit names the version, the quote and the replacement. Several edits from one read go in one change and apply in order.

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

The core, in the member's copy:

1. takes the document as it was at `version`, plus this lease's own edits since;
2. renders it as Markdown and finds each quote there, once; otherwise `anchorNotFound` or `anchorAmbiguous`, which now depend only on what the agent read, never on what the person typed since;
3. parses the replacement with the same vocabulary and writes the smallest change, text by characters, marks by ranges, blocks by type, under the leased client id, directly on the Yrs document, so that nodes, marks and attributes outside the vocabulary stay untouched;
4. folds that update into the live document, where Yjs merges it with everything written since the read.

The version is the document's state at the read. The core keeps it for an agent's open reads in a device-local, time-limited cache, no part of the view or the log, at about the size of the document's state (2.5 KB for the 2,000-insertion document of RFC 0013's F7). A read the core no longer holds is refused with an error such as `versionExpired`, and the agent reads again. Errors name no text, as the draft says.

## Options for D5

| | What is quoted | Looked up in | Fits GENESIS | Robust while the person types |
| --- | --- | --- | --- | --- |
| A | Markdown, replaced by Markdown (the draft) | The document at write time | Yes, focused edits | Refused whenever the person typed inside the quote (S2); the agent retries |
| B | Plain text within one block, plus calls to insert and delete blocks (the draft's B) | The document at write time | Yes | As A, and cannot change a mark or a block by quoting it |
| C | Markdown, replaced by Markdown | The version the agent read, merged by Yjs | Yes, focused edits | Applies, and keeps the person's typing (S3) |
| D | Node ids, with new content for the whole node | The document at write time | Not by itself: the node is replaced (S4) unless the core diffs it | Fails or rewrites when the node changed; models handle ids poorly [17] |
| E | An editing session: the agent moves a cursor to a quote and types, deletes and formats there | The live document, through relative positions | Yes | Robust (S5), but many calls per edit and one change per call or per D3 window |

- **A** is the draft. It works when the person is elsewhere, and fails, safely, exactly when they work on the same sentence.
- **B** is simpler to build and exact, but the agent translates what it read into calls, and marks are out of reach.
- **C** is A with the lookup moved to the read version. It costs a version token in the API and a cache of read states in the core, and it binds D5 to the D1 lease (S6).
- **D** is what Tiptap's toolkit does [15]. It needs ids in every block, which the document does not have, and edits whole nodes.
- **E** is closest to what the owner saw in Claude's artifacts. It can be built later on top of C, for agents that should be seen typing, once D8 brings presence.

## Recommendation

**C: Markdown quoted and Markdown replaced, looked up in the version the agent read, and written as a Yjs update made at that version under the agent's leased client id.**

- It keeps what the draft gets right: the agent quotes what it read, in the form it read it, as Claude's own tools do, and the core writes the smallest change.
- It removes what the owner doubts: whether an edit applies no longer depends on what the person typed since the read. The person's typing survives, even inside the agent's quote (S3).
- It needs D4's A or B, so that the core can render Markdown. With D4's C it still works, with plain-text quotes only.
- It adds three rules to RFC 0013 for a later revision, not in this ticket: a quote's ends fall on text, not inside markup; a block's type changes only when the replacement changes it, since that rewrites the block (S9); and an agent's edits for one field go through one lease in order, so the clock continues (S6).
- A cursor, a name and typing come later, with D8. The editor can animate the agent's text before then, since every character already names its writer.

The owner decides.

## Sources

1. RFC 0013 draft, PR #422, branch `carstenkoch/noc-411-rfc-rich-text-as-a-field-type-ncrichtext`, `docs/rfc/0013-rich-text.md`: D1, D3, D5, D8, F2, F6, F7.
2. GENESIS.md, "Never overwrite more than you change".
3. Yjs 13.6.33, `src/utils/Transaction.js`, where a document draws a new client id after a remote update added items under its own: <https://github.com/yjs/yjs/blob/v13.6.33/src/utils/Transaction.js>.
4. Anthropic, Claude Code tools reference, "Edit tool behavior": <https://code.claude.com/docs/en/tools-reference>.
5. Anthropic, text editor tool (`str_replace`): <https://platform.claude.com/docs/en/agents-and-tools/tool-use/text-editor-tool>.
6. Bram Cohen, "How Claude web is broken", 3 October 2025: <https://bramcohen.com/p/how-claude-web-is-broken>.
7. Aider, edit formats: <https://aider.chat/docs/more/edit-formats.html>.
8. Aider, "Unified diffs make GPT-4 Turbo 3X less lazy", 21 December 2023: <https://aider.chat/2023/12/21/unified-diffs.html>.
9. ProseMirror guide, "Document structure", "Indexing", "Transformations", "Rebasing", "Collaborative editing": <https://prosemirror.net/docs/guide/>; reference: <https://prosemirror.net/docs/ref/#transform>.
10. Marijn Haverbeke, "Collaborative Editing in ProseMirror", 2015: <https://marijnhaverbeke.nl/blog/collaborative-editing.html>.
11. Yjs, relative positions: <https://docs.yjs.dev/api/relative-positions>; delta format: <https://docs.yjs.dev/api/delta-format>.
12. y-prosemirror 1.3.7, README and `src/lib.js`, `src/plugins/sync-plugin.js`: <https://github.com/yjs/y-prosemirror/tree/v1.3.7>; Yjs awareness: <https://docs.yjs.dev/getting-started/adding-awareness>, <https://docs.yjs.dev/api/about-awareness>; TipTap collaboration caret: <https://tiptap.dev/docs/editor/extensions/functionality/collaboration-caret>.
13. Yrs, `StickyIndex` and `Assoc`: <https://docs.rs/yrs/latest/yrs/>.
14. Google Docs API, `documents.batchUpdate` and `WriteControl`: <https://developers.google.com/docs/api/reference/rest/v1/documents/batchUpdate>; best practices: <https://developers.google.com/docs/api/how-tos/best-practices>; structure and indexes: <https://developers.google.com/docs/api/concepts/structure>.
15. Tiptap AI Toolkit: overview <https://tiptap.dev/docs/ai/ai-toolkit/overview.md>, streaming <https://tiptap.dev/docs/ai/ai-toolkit/agents/streaming.md>, REST API with `tiptapEdit`, `_hash` and `concurrent_edit_conflict` <https://tiptap.dev/docs/ai/ai-toolkit/api-reference/rest-api.md>, tracked changes <https://tiptap.dev/docs/ai/ai-toolkit/agents/tracked-changes.md>.
16. BlockNote AI: <https://www.blocknotejs.org/docs/features/ai>, reference <https://www.blocknotejs.org/docs/features/ai/reference>, backend integration <https://www.blocknotejs.org/docs/features/ai/backend-integration>.
17. Liveblocks, "Building an AI copilot inside your Tiptap text editor", 21 November 2025: <https://liveblocks.io/blog/building-an-ai-copilot-inside-your-tiptap-text-editor>.
