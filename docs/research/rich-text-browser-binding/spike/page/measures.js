// The three measures of RFC 0013 ("An editor that cannot show a node"), as
// TipTap extensions and a binding guard, carried over from the NOC-437 spike
// (docs/research/rich-text-unknown-nodes on its branch) and made to run in a
// real editor: placeholders render, and copy and paste round-trip them.
import { Extension, Mark, Node } from "@tiptap/core";
import * as Y from "yjs";

const markName = (key) => key.replace(/--.*$/, "");

/** Everything a fragment holds that `schema` does not know. */
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

// Attributes declared without a default of their own, kept in the DOM as one
// data attribute, so that copy and paste inside the editor keeps them.
const keptAttrs = (names) =>
  Object.fromEntries(
    [...names].map((a) => [
      a,
      {
        default: null,
        parseHTML: (el) => JSON.parse(el.dataset.ncAttrs ?? "{}")[a] ?? null,
        renderHTML: () => ({}),
      },
    ]),
  );

const attrsData = (attrs) => ({ "data-nc-attrs": JSON.stringify(attrs) });

/**
 * TipTap extensions with a placeholder for each unknown node, mark and
 * attribute of `fragment`, for an editor whose own extensions make `schema`.
 * `roundTrip` false leaves out parseHTML, as the NOC-437 spike did.
 */
export const widen = (fragment, schema, { roundTrip = true } = {}) => {
  const u = unknownParts(fragment, schema);
  const ext = [];
  for (const [name, n] of u.nodes) {
    const tag = n.inline ? "span" : "div";
    ext.push(
      Node.create({
        name,
        group: n.inline ? "inline" : "block",
        inline: n.inline,
        atom: true,
        content: n.leaf ? "" : n.textInside ? "inline*" : "block*",
        addAttributes: () => keptAttrs(n.attrs),
        parseHTML: () =>
          roundTrip ? [{ tag: `${tag}[data-nc-unknown="${name}"]` }] : [],
        renderHTML: ({ node }) =>
          n.leaf
            ? [
                tag,
                {
                  "data-nc-unknown": name,
                  ...attrsData(node.attrs),
                  class: "nc-unknown",
                },
              ]
            : [
                tag,
                {
                  "data-nc-unknown": name,
                  ...attrsData(node.attrs),
                  class: "nc-unknown",
                },
                0,
              ],
      }),
    );
  }
  for (const [name, a] of u.marks) {
    ext.push(
      Mark.create({
        name,
        addAttributes: () => keptAttrs(a),
        parseHTML: () =>
          roundTrip ? [{ tag: `span[data-nc-unknown-mark="${name}"]` }] : [],
        renderHTML: ({ mark }) => [
          "span",
          {
            "data-nc-unknown-mark": name,
            ...attrsData(mark.attrs),
            class: "nc-unknown-mark",
          },
          0,
        ],
      }),
    );
  }
  if (u.attrs.size > 0) {
    ext.push(
      Extension.create({
        name: "ncKeepAttrs",
        addGlobalAttributes: () =>
          [...u.attrs].map(([type, names]) => ({
            types: [type],
            attributes: Object.fromEntries(
              [...names].map((k) => [
                k,
                {
                  default: null,
                  parseHTML: roundTrip
                    ? (el) => el.getAttribute(`data-nc-${k}`)
                    : () => null,
                  renderHTML: (attrs) =>
                    attrs[k] == null ? {} : { [`data-nc-${k}`]: attrs[k] },
                },
              ]),
            ),
          })),
      }),
    );
  }
  return { extensions: ext, unknown: u };
};

/**
 * The guard: what a transaction deletes that `getSchema()` does not know,
 * an element, a format or an attribute. Placeholders are in the schema, so
 * deleting one on purpose passes.
 */
export const guardFor = (getSchema) => (tr) => {
  const schema = getSchema();
  if (!schema) return [];
  const flagged = [];
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
  return flagged;
};
