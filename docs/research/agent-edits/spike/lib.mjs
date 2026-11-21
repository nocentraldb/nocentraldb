// Helpers for the NOC-436 spike. Two or more Yjs documents stand for members;
// the agent edits through ProseMirror transforms, and y-prosemirror's
// updateYFragment turns the edited ProseMirror document into Yjs operations,
// as the editor binding does in a browser.
import { schema } from "prosemirror-schema-basic";
import { Transform } from "prosemirror-transform";
import { initProseMirrorDoc, updateYFragment } from "y-prosemirror";
import * as Y from "yjs";

export { schema, Y };

export const FIELD = "default";

export const newMember = (clientID) => {
  const doc = new Y.Doc();
  doc.clientID = clientID;
  return doc;
};

export const frag = (doc) => doc.getXmlFragment(FIELD);

// Sends what each document lacks to the other, as a relay would.
export const sync = (...docs) => {
  for (const from of docs) {
    for (const to of docs) {
      if (from === to) continue;
      Y.applyUpdate(to, Y.encodeStateAsUpdate(from, Y.encodeStateVector(to)));
    }
  }
};

export const paragraph = (text) => {
  const p = new Y.XmlElement("paragraph");
  const t = new Y.XmlText();
  t.insert(0, text);
  p.insert(0, [t]);
  return p;
};

export const seed = (doc, texts) => {
  frag(doc).insert(
    0,
    texts.map((t) => paragraph(t)),
  );
};

// The document as ProseMirror sees it.
export const pmDoc = (doc) => initProseMirrorDoc(frag(doc), schema);

export const text = (doc) =>
  pmDoc(doc)
    .doc.children.map((b) => b.textContent)
    .join(" | ");

// The text of block i as Yjs holds it, for a person's keystrokes.
export const blockText = (doc, i) => frag(doc).get(i).get(0);

// Finds a plain-text quote in the ProseMirror document. Returns every match
// as ProseMirror positions { from, to }.
export const findQuote = (pm, quote) => {
  const found = [];
  pm.descendants((node, pos) => {
    if (!node.isTextblock) return true;
    const t = node.textContent;
    for (let i = t.indexOf(quote); i >= 0; i = t.indexOf(quote, i + 1)) {
      found.push({ from: pos + 1 + i, to: pos + 1 + i + quote.length });
    }
    return false;
  });
  return found;
};

// Applies a ProseMirror edit to a Yjs document the way the editor binding
// does: build the new ProseMirror document, then let y-prosemirror write the
// difference. `edit` receives a Transform and the current ProseMirror doc.
export const editThroughProseMirror = (doc, edit) => {
  const { doc: pm, meta } = pmDoc(doc);
  const tr = new Transform(pm);
  const result = edit(tr, pm);
  if (result === false) return false;
  doc.transact(() => updateYFragment(doc, frag(doc), tr.doc, meta));
  return true;
};

// Replaces a unique plain-text quote with a replacement, resolved against the
// document as it is now (the "resolve at write time" approach).
export const quoteEditNow = (doc, quote, replacement) =>
  editThroughProseMirror(doc, (tr, pm) => {
    const hits = findQuote(pm, quote);
    if (hits.length !== 1) return false;
    insertText(tr, replacement, hits[0].from, hits[0].to);
  });

// Transaction.insertText for a bare Transform: the text keeps the marks found
// at the start of the range.
export const insertText = (tr, str, from, to = from) => {
  const marks =
    tr.doc.resolve(from).marksAcross(tr.doc.resolve(to)) ??
    tr.doc.resolve(from).marks();
  if (str === "") tr.delete(from, to);
  else tr.replaceWith(from, to, schema.text(str, marks));
};

// The "edit against the version you read" approach. `read` is what the agent
// got when it read: the document's state as a Yjs update. The edit is made in
// a fork at that state and the fork's update is merged into the live copy.
// `writer` is the client id the agent leased (RFC 0013's D1); the live copy is
// the member's folded document, which never writes under that id itself:
// Yjs gives a document a new client id when an update brings items under its
// own id that it did not write. `ownSince` are the agent's own updates
// written after the read, so that the fork continues the agent's clock
// instead of reusing it (RFC 0013's F6).
export const readVersion = (doc) => Y.encodeStateAsUpdate(doc);

export const quoteEditAtRead = (
  live,
  writer,
  read,
  quote,
  replacement,
  ownSince = [],
) => {
  // Loading the writer's own items into a document that already carries the
  // writer's id would make Yjs draw a new id; so load first, then take the id.
  const fork = newMember(0);
  fork.gc = false;
  Y.applyUpdate(fork, read);
  for (const u of ownSince) Y.applyUpdate(fork, u);
  fork.clientID = writer;
  const before = Y.encodeStateVector(fork);
  const ok = quoteEditNow(fork, quote, replacement);
  if (!ok) return null;
  const update = Y.encodeStateAsUpdate(fork, before);
  // The delete set of a diff from a state vector names every deletion the
  // fork holds; Yjs applies deletions idempotently, so this is harmless.
  Y.applyUpdate(live, update);
  return update;
};

// Who wrote what in block i: runs of [client id, text], deleted runs left out.
export const authors = (doc, i) => {
  const runs = [];
  for (let item = blockText(doc, i)._start; item; item = item.right) {
    if (item.deleted || !item.content.str) continue;
    const last = runs[runs.length - 1];
    if (last && last[0] === item.id.client) last[1] += item.content.str;
    else runs.push([item.id.client, item.content.str]);
  }
  return runs;
};

export const report = (label, value) => {
  console.log(
    `${label.padEnd(44)} ${typeof value === "string" ? value : JSON.stringify(value)}`,
  );
};
