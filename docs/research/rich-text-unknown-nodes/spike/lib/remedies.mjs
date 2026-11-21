// Ways to keep what an editor cannot interpret: widen the editor's schema
// from the document before it opens, rebuild it when a new kind arrives, and a
// guard that spots an editor's write deleting parts its schema does not know.
import { Mark, Node } from "@tiptap/core";
import * as YT from "@tiptap/y-tiptap";
import { Schema } from "prosemirror-model";
import * as YP from "y-prosemirror";
import { prosemirrorJSONToYXmlFragment } from "y-prosemirror";
import * as Y from "yjs";
import { tiptap, typeAt, yProsemirror } from "./editors.mjs";
import { writeNewDoc } from "./scenario.mjs";
import {
  newDocJSON,
  newMember,
  newSchema,
  oldMember,
  oldSchema,
  survivors,
  sync,
} from "./setup.mjs";

const markName = (key) => key.replace(/--.*$/, "");

// Everything in a fragment the schema does not know: element names with where
// they sit and their attributes, mark names with their attributes, and
// attributes of known elements.
export const unknownParts = (fragment, schema) => {
  const nodes = new Map();
  const marks = new Map();
  const attrs = new Map();
  const visit = (type, parentHasText) => {
    for (const child of type.toArray()) {
      if (child instanceof Y.XmlText) {
        for (const op of child.toDelta()) {
          for (const [key, value] of Object.entries(op.attributes ?? {})) {
            const name = markName(key);
            if (schema.marks[name]) continue;
            const seen = marks.get(name) ?? new Set();
            for (const a of Object.keys(value ?? {})) seen.add(a);
            marks.set(name, seen);
          }
        }
        continue;
      }
      const name = child.nodeName;
      const own = Object.keys(child.getAttributes());
      const hasText = child.toArray().some((c) => c instanceof Y.XmlText);
      if (!schema.nodes[name]) {
        const n = nodes.get(name) ?? {
          inline: parentHasText,
          textInside: hasText,
          leaf: child.length === 0,
          attrs: new Set(),
        };
        for (const a of own) n.attrs.add(a);
        nodes.set(name, n);
      } else {
        const declared = schema.nodes[name].spec.attrs ?? {};
        for (const a of own) {
          if (declared[a]) continue;
          const seen = attrs.get(name) ?? new Set();
          seen.add(a);
          attrs.set(name, seen);
        }
      }
      visit(child, hasText);
    }
  };
  visit(fragment, false);
  return { nodes, marks, attrs };
};

const nullAttrs = (names) =>
  Object.fromEntries([...names].map((a) => [a, { default: null }]));

const opaqueSpec = (name, n) => ({
  group: n.inline ? "inline" : "block",
  inline: n.inline,
  atom: true,
  content: n.leaf ? "" : n.textInside ? "inline*" : "block*",
  attrs: nullAttrs(n.attrs),
  toDOM: () => [n.inline ? "span" : "div", { "data-unknown": name }, 0],
});

// A ProseMirror schema widened by placeholders for what the fragment holds.
export const widenSchema = (base, fragment) => {
  const u = unknownParts(fragment, base);
  let nodes = base.spec.nodes;
  for (const [name, attrs] of u.attrs) {
    const spec = nodes.get(name);
    nodes = nodes.update(name, {
      ...spec,
      attrs: { ...spec.attrs, ...nullAttrs(attrs) },
    });
  }
  for (const [name, n] of u.nodes)
    nodes = nodes.addToEnd(name, opaqueSpec(name, n));
  let marks = base.spec.marks;
  for (const [name, a] of u.marks) {
    marks = marks.addToEnd(name, {
      attrs: nullAttrs(a),
      toDOM: () => ["span", 0],
    });
  }
  return new Schema({ nodes, marks });
};

// The same widening, as TipTap extensions for an app built on StarterKit.
export const widenTiptap = (fragment) => {
  const u = unknownParts(fragment, oldSchema);
  const ext = [];
  for (const [name, n] of u.nodes) {
    const spec = opaqueSpec(name, n);
    ext.push(
      Node.create({
        name,
        group: spec.group,
        inline: spec.inline,
        atom: true,
        content: spec.content,
        addAttributes: () =>
          Object.fromEntries([...n.attrs].map((a) => [a, { default: null }])),
        renderHTML: () => spec.toDOM(),
      }),
    );
  }
  for (const [name, a] of u.marks) {
    ext.push(
      Mark.create({
        name,
        addAttributes: () =>
          Object.fromEntries([...a].map((k) => [k, { default: null }])),
        renderHTML: () => ["span", 0],
      }),
    );
  }
  if (u.attrs.size > 0) {
    ext.push(
      Node.create({ name: "ncdbKeepAttrs" }).extend({
        addGlobalAttributes: () =>
          [...u.attrs].map(([type, names]) => ({
            types: [type],
            attributes: Object.fromEntries(
              [...names].map((k) => [k, { default: null }]),
            ),
          })),
      }),
    );
  }
  return ext.filter((e) => e.name !== "ncdbKeepAttrs" || u.attrs.size > 0);
};

// Flags every deletion in an editor's own transaction of a part the editor's
// schema does not know. Yjs cannot undo a transaction, so a binding that uses
// it refuses to write the update and reloads the document instead.
export const guard = (doc, schema) => {
  const flagged = [];
  const onTx = (tr) => {
    if (!tr.local) return;
    Y.iterateDeletedStructs(tr, tr.deleteSet, (s) => {
      if (!(s instanceof Y.Item)) return;
      const c = s.content;
      if (c instanceof Y.ContentType && c.type instanceof Y.XmlElement) {
        if (!schema.nodes[c.type.nodeName])
          flagged.push(`node ${c.type.nodeName}`);
      } else if (c instanceof Y.ContentFormat) {
        if (!schema.marks[markName(c.key)]) flagged.push(`mark ${c.key}`);
      } else if (s.parentSub !== null && s.parent instanceof Y.XmlElement) {
        const declared = schema.nodes[s.parent.nodeName]?.spec.attrs ?? {};
        if (!declared[s.parentSub]) flagged.push(`attribute ${s.parentSub}`);
      }
    });
  };
  doc.on("afterTransaction", onTx);
  return { flagged, stop: () => doc.off("afterTransaction", onTx) };
};

const lostParts = (s) => Object.keys(s).filter((k) => !s[k]);

const openReader = () => {
  const writer = writeNewDoc();
  const reader = oldMember();
  sync(writer, reader);
  return { writer, reader };
};

const widenedYProsemirror = () => {
  const { writer, reader } = openReader();
  const schema = widenSchema(oldSchema, reader.getXmlFragment("default"));
  const ed = yProsemirror(reader, schema);
  const typed = [
    typeAt(ed.view, "Intro.", "Hi. "),
    typeAt(ed.view, "Centered.", "Now "),
  ];
  sync(reader, writer);
  ed.destroy();
  return {
    widenedWith: Object.keys(schema.nodes).filter((n) => !oldSchema.nodes[n]),
    typed: typed.map((t) => t.typed),
    readerLost: lostParts(survivors(reader)),
    writerLost: lostParts(survivors(writer)),
  };
};

const widenedTiptap = () => {
  const { writer, reader } = openReader();
  const ed = tiptap(reader, {
    enableContentCheck: true,
    extensions: widenTiptap(reader.getXmlFragment("default")),
  });
  const typed = [
    typeAt(ed.view, "Intro.", "Hi. "),
    typeAt(ed.view, "Centered.", "Now "),
  ];
  sync(reader, writer);
  ed.destroy();
  return {
    typed: typed.map((t) => t.typed),
    errors: ed.errors.length,
    readerLost: lostParts(survivors(reader)),
    writerLost: lostParts(survivors(writer)),
  };
};

// The newer app adds a node of a kind the older app's editor has not seen
// while that editor is open.
const addFigure = (writer) => {
  const figure = new Y.XmlElement("figure");
  figure.setAttribute("src", "img-7");
  writer.getXmlFragment("default").insert(1, [figure]);
};

const lateArrivalYProsemirror = () => {
  const { writer, reader } = openReader();
  const ed = yProsemirror(
    reader,
    widenSchema(oldSchema, reader.getXmlFragment("default")),
  );
  addFigure(writer);
  sync(writer, reader);
  const kept = reader.getXmlFragment("default").toString().includes("<figure");
  ed.destroy();
  return { figureKeptInReader: kept };
};

const lateArrivalTiptap = () => {
  const { writer, reader } = openReader();
  const open = () =>
    tiptap(reader, {
      enableContentCheck: true,
      extensions: widenTiptap(reader.getXmlFragment("default")),
    });
  let ed = open();
  addFigure(writer);
  sync(writer, reader);
  const errorsOnArrival = ed.errors.length;
  const typedWhileStale = typeAt(ed.view, "Intro.", "Hi. ").typed;
  // The app answers contentError by rebuilding the editor with a schema
  // widened from the document as it is now.
  ed.destroy();
  ed = open();
  const typedAfterRebuild = typeAt(ed.view, "Intro.", "Hi. ").typed;
  sync(reader, writer);
  const figureKept = writer
    .getXmlFragment("default")
    .toString()
    .includes("<figure");
  ed.destroy();
  return {
    errorsOnArrival,
    typedWhileStale,
    typedAfterRebuild,
    figureKeptInWriter: figureKept,
    writerLost: lostParts(survivors(writer)),
  };
};

const guardOnPlainYProsemirror = () => {
  const { reader } = openReader();
  const g = guard(reader, oldSchema);
  const ed = yProsemirror(reader, oldSchema);
  typeAt(ed.view, "Centered.", "Now ");
  ed.destroy();
  g.stop();
  return { flagged: g.flagged };
};

// The guard must not fire on ordinary editing by an app that knows every part.
const guardOnFullSchema = () => {
  const doc = writeNewDoc();
  const g = guard(doc, newSchema);
  const ed = yProsemirror(doc, newSchema);
  const v = ed.view;
  typeAt(v, "Intro.", "Hi. ");
  typeAt(v, "Centered.", "Now ");
  let calloutPos = null;
  v.state.doc.forEach((n, p) => {
    if (n.type.name === "callout") calloutPos = p;
  });
  const callout = v.state.doc.nodeAt(calloutPos);
  v.dispatch(v.state.tr.delete(calloutPos, calloutPos + callout.nodeSize));
  v.dispatch(
    v.state.tr.removeMark(
      0,
      v.state.doc.content.size,
      newSchema.marks.highlight,
    ),
  );
  ed.destroy();
  g.stop();
  return { flagged: g.flagged };
};

// A document whose only unknown part is an attribute of a known node.
const attributeOnlyTiptap = () => {
  const writer = newMember();
  prosemirrorJSONToYXmlFragment(
    newSchema,
    { ...newDocJSON, content: [newDocJSON.content[0], newDocJSON.content[2]] },
    writer.getXmlFragment("default"),
  );
  const reader = oldMember();
  sync(writer, reader);
  const ed = tiptap(reader, { enableContentCheck: true });
  const typed = typeAt(ed.view, "Centered.", "Now ").typed;
  sync(reader, writer);
  ed.destroy();
  return {
    document: writer.getXmlFragment("default").toString(),
    errors: ed.errors.length,
    typed,
    alignKeptInWriter: writer
      .getXmlFragment("default")
      .toString()
      .includes('align="center"'),
  };
};

// A viewer that may not edit: y-prosemirror projects the document all the same.
const readOnlyYProsemirror = () => {
  const { reader } = openReader();
  const ed = yProsemirror(reader, oldSchema, { editable: () => false });
  const lost = lostParts(survivors(reader));
  ed.destroy();
  return { readerLost: lost };
};

// No editor at all: converting the fragment to a ProseMirror node with the
// older schema, as a headless reader or an agent tool would.
const headless = () => {
  const out = {};
  for (const [lib, mod] of [
    ["y-prosemirror", YP],
    ["y-tiptap", YT],
  ]) {
    for (const fn of [
      "yXmlFragmentToProseMirrorRootNode",
      "initProseMirrorDoc",
      "yXmlFragmentToProsemirrorJSON",
    ]) {
      const { reader } = openReader();
      mod[fn](reader.getXmlFragment("default"), oldSchema);
      out[`${lib} ${fn}`] = lostParts(survivors(reader));
    }
  }
  return out;
};

export const runRemedies = () => ({
  "headless conversion with the old schema": headless(),
  "read-only y-prosemirror with the old schema": readOnlyYProsemirror(),
  "attribute only, TipTap with enableContentCheck": attributeOnlyTiptap(),
  "widened schema, y-prosemirror": widenedYProsemirror(),
  "widened schema, TipTap with enableContentCheck": widenedTiptap(),
  "late arrival, widened y-prosemirror": lateArrivalYProsemirror(),
  "late arrival, TipTap with enableContentCheck, rebuilt on contentError":
    lateArrivalTiptap(),
  "guard, plain y-prosemirror with the old schema": guardOnPlainYProsemirror(),
  "guard, ordinary edits with the full schema": guardOnFullSchema(),
});
