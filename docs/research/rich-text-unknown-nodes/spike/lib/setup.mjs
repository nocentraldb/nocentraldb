// Shared setup for the NOC-437 spike: a DOM for ProseMirror's EditorView in
// Node, the two schemas, the document the newer app writes, and helpers that
// sync two Yjs documents and describe a fragment without any schema.
import { JSDOM } from "jsdom";
import { Schema } from "prosemirror-model";
import * as Y from "yjs";

const dom = new JSDOM("<!doctype html><html><body></body></html>", {
  pretendToBeVisual: true,
});
for (const key of [
  "window",
  "document",
  "navigator",
  "Node",
  "HTMLElement",
  "Element",
  "Text",
  "DOMParser",
  "MutationObserver",
  "getComputedStyle",
  "requestAnimationFrame",
  "cancelAnimationFrame",
]) {
  if (globalThis[key] === undefined) {
    Object.defineProperty(globalThis, key, {
      value: key === "window" ? dom.window : dom.window[key],
      configurable: true,
      writable: true,
    });
  }
}
// jsdom has no layout; ProseMirror asks for rectangles when it scrolls.
dom.window.Element.prototype.getClientRects = () => [];
dom.window.Element.prototype.getBoundingClientRect = () => ({
  top: 0,
  left: 0,
  right: 0,
  bottom: 0,
  width: 0,
  height: 0,
});
dom.window.document.elementFromPoint = () => null;
dom.window.Range.prototype.getClientRects = () => [];
dom.window.Range.prototype.getBoundingClientRect = () => ({
  top: 0,
  left: 0,
  right: 0,
  bottom: 0,
  width: 0,
  height: 0,
});

const baseNodes = {
  doc: { content: "block+" },
  paragraph: {
    group: "block",
    content: "inline*",
    parseDOM: [{ tag: "p" }],
    toDOM: () => ["p", 0],
  },
  text: { group: "inline" },
};
const baseMarks = {
  bold: { parseDOM: [{ tag: "b" }], toDOM: () => ["b", 0] },
};

// The older app: paragraphs, text and bold. Nothing else.
export const oldSchema = new Schema({ nodes: baseNodes, marks: baseMarks });

// The newer app adds a block node, an inline node, a mark, and an attribute
// on a node the older app knows.
export const newSchema = new Schema({
  nodes: {
    doc: baseNodes.doc,
    paragraph: {
      ...baseNodes.paragraph,
      attrs: { align: { default: null } },
    },
    text: baseNodes.text,
    callout: {
      group: "block",
      content: "inline*",
      attrs: { tone: { default: "info" } },
      toDOM: () => ["aside", 0],
    },
    chip: {
      group: "inline",
      inline: true,
      atom: true,
      attrs: { ref: { default: null } },
      toDOM: () => ["span", { class: "chip" }],
    },
  },
  marks: {
    ...baseMarks,
    highlight: {
      attrs: { color: { default: "yellow" } },
      toDOM: () => ["mark", 0],
    },
  },
});

// The document as the newer app writes it, one block per concern.
export const newDocJSON = {
  type: "doc",
  content: [
    { type: "paragraph", content: [{ type: "text", text: "Intro." }] },
    {
      type: "callout",
      attrs: { tone: "warn" },
      content: [{ type: "text", text: "Careful." }],
    },
    {
      type: "paragraph",
      attrs: { align: "center" },
      content: [{ type: "text", text: "Centered." }],
    },
    {
      type: "paragraph",
      content: [
        { type: "text", text: "Some " },
        {
          type: "text",
          text: "marked",
          marks: [{ type: "highlight", attrs: { color: "green" } }],
        },
        { type: "text", text: " text." },
      ],
    },
    {
      type: "paragraph",
      content: [
        { type: "text", text: "See " },
        { type: "chip", attrs: { ref: "obj-42" } },
        { type: "text", text: " here." },
      ],
    },
  ],
};

// Two members with fixed client ids, as in RFC 0013's spike.
export const newMember = () => {
  const doc = new Y.Doc();
  doc.clientID = 1;
  return doc;
};
export const oldMember = () => {
  const doc = new Y.Doc();
  doc.clientID = 2;
  return doc;
};

export const sync = (from, to) => {
  Y.applyUpdate(to, Y.encodeStateAsUpdate(from, Y.encodeStateVector(to)));
};

// What the fragment holds, read from Yjs alone, with no schema.
export const xml = (doc) => doc.getXmlFragment("default").toString();

// Which parts of the newer app's document a fragment still holds.
export const survivors = (doc) => {
  const s = xml(doc);
  return {
    callout: s.includes("<callout") && s.includes("Careful."),
    calloutAttr: s.includes('tone="warn"'),
    alignAttr: s.includes('align="center"'),
    highlightMark: s.includes("<highlight"),
    highlightText: s.includes("marked"),
    chip: s.includes("<chip") && s.includes('ref="obj-42"'),
    chipParagraphText: s.includes("See ") && s.includes(" here."),
  };
};

// Every struct a transaction deleted, described by what it was.
export const describeDeletes = (transaction) => {
  const out = [];
  Y.iterateDeletedStructs(transaction, transaction.deleteSet, (struct) => {
    if (!(struct instanceof Y.Item)) return;
    const c = struct.content;
    if (c instanceof Y.ContentType) {
      out.push({
        kind: "element",
        name: c.type.nodeName ?? c.type.constructor.name,
      });
    } else if (c instanceof Y.ContentFormat) {
      out.push({ kind: "format", key: c.key });
    } else if (struct.parentSub !== null) {
      out.push({ kind: "attribute", key: struct.parentSub });
    } else if (c instanceof Y.ContentString) {
      out.push({ kind: "text", length: c.str.length });
    }
  });
  return out;
};
