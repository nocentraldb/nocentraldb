// The newer app of the NOC-437 spike: it knows a block `callout` (tone), an
// inline atom `chip` (ref), a mark `highlight` (color) and an `align` on
// paragraphs, none of which the older app's StarterKit knows. It writes
// without an editor, straight into a Y.Doc with a client id of its own.
import { Schema } from "@tiptap/pm/model";
import { prosemirrorJSONToYXmlFragment } from "@tiptap/y-tiptap";
import * as Y from "yjs";

export const newerSchema = new Schema({
  nodes: {
    doc: { content: "block+" },
    paragraph: {
      group: "block",
      content: "inline*",
      attrs: { align: { default: null } },
      toDOM: () => ["p", 0],
    },
    text: { group: "inline" },
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
    bold: { toDOM: () => ["strong", 0] },
    highlight: {
      attrs: { color: { default: "yellow" } },
      toDOM: () => ["mark", 0],
    },
  },
});

const text = (t, marks) => ({
  type: "text",
  text: t,
  ...(marks ? { marks } : {}),
});

export const newerDocJSON = {
  type: "doc",
  content: [
    { type: "paragraph", content: [text("Intro.")] },
    { type: "callout", attrs: { tone: "warn" }, content: [text("Careful.")] },
    {
      type: "paragraph",
      attrs: { align: "center" },
      content: [text("Centered.")],
    },
    {
      type: "paragraph",
      content: [
        text("Some "),
        text("marked", [{ type: "highlight", attrs: { color: "green" } }]),
        text(" text."),
      ],
    },
    {
      type: "paragraph",
      content: [
        text("See "),
        { type: "chip", attrs: { ref: "obj-42" } },
        text(" here."),
      ],
    },
  ],
};

/** What the fragment holds, read from Yjs alone. */
export const xml = (doc) => doc.getXmlFragment("default").toString();

/** Which parts of the newer app's document the fragment still holds. */
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

export const lost = (doc) =>
  Object.entries(survivors(doc))
    .filter(([, kept]) => !kept)
    .map(([name]) => name);

/** The newer app, as a member with client id 1, writing through `core`. */
export const newerApp = (core) => {
  const doc = new Y.Doc();
  doc.clientID = 1;
  const send = async (fn) => {
    Y.applyUpdate(doc, await core.state());
    const before = Y.encodeStateVector(doc);
    fn(doc.getXmlFragment("default"));
    await core.write(Y.encodeStateAsUpdate(doc, before), { clientID: 1 });
  };
  return {
    doc,
    writeDocument: () =>
      send((fragment) =>
        prosemirrorJSONToYXmlFragment(newerSchema, newerDocJSON, fragment),
      ),
    // A kind of node the older app has not seen yet: a figure, at block 1.
    addFigure: () =>
      send((fragment) => {
        const figure = new Y.XmlElement("figure");
        figure.setAttribute("src", "img-7");
        fragment.insert(1, [figure]);
      }),
  };
};
