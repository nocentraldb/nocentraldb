// Writes the Yjs/Yrs agreement vectors of RFC 0013 ("How rich text meets
// the plugin contract", C3) in this directory, with Yjs itself, as the
// editor writes: updates of two members on a TipTap-shaped document, the
// whole state a Yjs document ends in after applying them all, and its XML.
// tests/rich_text_agreement.rs folds the updates with the core, natively
// and in wasm32, in several orders, and must reach the same state bytes.
// The gates do not run this script; an upgrade of Yjs or Yrs must pass the
// committed files unchanged (RFC 0013).
//
// Run: node generate.mjs, with a node_modules beside it or above it that
// holds yjs@13.6.33 and its lib0 (0.2.120), e.g. a symbolic link to one
// made with `npm install yjs@13.6.33` elsewhere.

import { createHash } from "node:crypto";
import { writeFileSync } from "node:fs";
import * as Y from "yjs";

const here = new URL(".", import.meta.url);

/** RFC 0013's client id: the low 53 bits of SHA-256(label, member, nonce). */
function clientId(member, nonce) {
  const hash = createHash("sha256")
    .update("nocentraldb/yjs-client-id")
    .update(member)
    .update(nonce)
    .digest();
  return Number(hash.readBigUInt64BE(24) & ((1n << 53n) - 1n));
}

const NONCE = Buffer.from([1, 2, 3, 4, 5, 6, 7, 8]);
const ALICE = Buffer.alloc(32, 0x0a);
const BOB = Buffer.alloc(32, 0x0b);

function member(id) {
  const doc = new Y.Doc();
  doc.clientID = clientId(id, NONCE);
  const updates = [];
  doc.on("update", (update, origin) => {
    if (origin !== "sync") updates.push(update);
  });
  return { doc, body: doc.getXmlFragment("default"), updates };
}

function sync(from, to) {
  Y.applyUpdate(
    to.doc,
    Y.encodeStateAsUpdate(from.doc, Y.encodeStateVector(to.doc)),
    "sync",
  );
}

function block(tag, text, attrs = {}) {
  const el = new Y.XmlElement(tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
  el.insert(0, [new Y.XmlText(text)]);
  return el;
}

const a = member(ALICE);
const b = member(BOB);

// Two paragraphs and a heading; Bob sees them.
a.body.insert(0, [block("heading", "Notes", { level: "1" })]);
a.body.insert(1, [block("paragraph", "We ship on Friday.")]);
a.body.insert(2, [block("paragraph", "Second paragraph.")]);
sync(a, b);
// Alice makes a word bold; Bob types an insertion.
a.body.get(1).get(0).format(11, 6, { bold: true });
b.body.get(2).get(0).insert(6, " and better");
// Both set the heading's level at once.
a.body.get(0).setAttribute("level", "2");
b.body.get(0).setAttribute("level", "3");
// Alice deletes a paragraph while Bob types into it.
a.body.delete(2, 1);
b.body.get(2).get(0).insert(0, "Typed: ");
// Alice deletes characters she sees; then everyone syncs.
a.body.get(1).get(0).delete(0, 3);
sync(a, b);
sync(b, a);

const all = [
  ...a.updates.map((u) => ["alice", u]),
  ...b.updates.map((u) => ["bob", u]),
];
const check = new Y.Doc();
for (const [, u] of all) Y.applyUpdate(check, u);
const state = Y.encodeStateAsUpdate(check);
const ok = Buffer.from(state).equals(Buffer.from(Y.encodeStateAsUpdate(a.doc)));
if (!ok)
  throw new Error(
    "the members' documents differ from the one that applied all",
  );

const hex = (bytes) => Buffer.from(bytes).toString("hex");
const lines = all.map(([who, u]) => `${who} ${hex(u)}`).join("\n");
writeFileSync(new URL("updates.txt", here), `${lines}\n`);
writeFileSync(new URL("state.hex", here), `${hex(state)}\n`);
writeFileSync(
  new URL("document.xml", here),
  `${check.getXmlFragment("default").toString()}\n`,
);
