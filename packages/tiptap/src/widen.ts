// Measure 1 of RFC 0013, "An editor that cannot show a node": a schema
// widened from the document. Before the editor opens, the field's document
// is scanned, and each node, mark and attribute the editor does not know
// gets a placeholder type, so the editor shows it, keeps it and keeps the
// rest editable: a node of that name, inline or block as it sits, with its
// attributes declared without a default of their own; a mark with its
// attributes; an extra attribute on a known node. Each placeholder has
// parse rules, so copying and pasting it keeps it (NOC-455). The scan
// reads the Yjs tree and never converts it through a ProseMirror schema,
// which would delete what that schema lacks (RFC 0013, F16).
import {
  type AnyExtension,
  Extension,
  getSchema,
  Mark,
  Node,
} from "@tiptap/core";
import * as Y from "yjs";

/** An attribute kept as it is: a data attribute in HTML, its value as JSON. */
const attribute = (key: string) => ({
  default: null,
  parseHTML: (element: HTMLElement) => {
    const raw = element.getAttribute(`data-nc-${key}`);
    if (raw === null) return null;
    try {
      return JSON.parse(raw);
    } catch {
      return raw;
    }
  },
  renderHTML: (attributes: Record<string, unknown>) =>
    attributes[key] === null || attributes[key] === undefined
      ? {}
      : { [`data-nc-${key}`]: JSON.stringify(attributes[key]) },
});

const attributes = (keys: Iterable<string>) =>
  Object.fromEntries([...keys].map((key) => [key, attribute(key)]));

interface UnknownNode {
  inline: boolean;
  text: boolean;
  children: boolean;
  attributes: Set<string>;
}

/** The extensions that widen the schema `base` builds for `doc`: none when the editor knows every part of it. */
export const widen = (doc: Y.Doc, base: AnyExtension[]): AnyExtension[] => {
  const schema = getSchema(base);
  const nodes = new Map<string, UnknownNode>();
  const marks = new Map<string, Set<string>>();
  const extra = new Map<string, Set<string>>();

  const walk = (parent: Y.XmlFragment | Y.XmlElement, inText: boolean) => {
    const children = parent.toArray();
    const mixed = children.some((child) => child instanceof Y.XmlText);
    for (const child of children) {
      if (child instanceof Y.XmlText) {
        for (const op of child.toDelta() as { attributes?: object }[]) {
          for (const [mark, value] of Object.entries(op.attributes ?? {})) {
            if (schema.marks[mark]) continue;
            const keys = marks.get(mark) ?? new Set<string>();
            if (value && typeof value === "object") {
              for (const key of Object.keys(value)) keys.add(key);
            }
            marks.set(mark, keys);
          }
        }
        continue;
      }
      if (!(child instanceof Y.XmlElement)) continue;
      const name = child.nodeName;
      const known = schema.nodes[name];
      const keys = Object.keys(child.getAttributes());
      if (known) {
        const declared = known.spec.attrs ?? {};
        const undeclared = keys.filter((key) => !(key in declared));
        if (undeclared.length > 0) {
          const set = extra.get(name) ?? new Set<string>();
          for (const key of undeclared) set.add(key);
          extra.set(name, set);
        }
        walk(child, Boolean(known.isTextblock));
        continue;
      }
      const grandchildren = child.toArray();
      const found = nodes.get(name) ?? {
        inline: inText || mixed,
        text: false,
        children: false,
        attributes: new Set<string>(),
      };
      for (const key of keys) found.attributes.add(key);
      found.text ||= grandchildren.some((c) => c instanceof Y.XmlText);
      found.children ||= grandchildren.length > 0;
      nodes.set(name, found);
      walk(child, found.text);
    }
  };
  walk(doc.getXmlFragment("default"), false);

  const widened: AnyExtension[] = [];
  for (const [name, found] of nodes) {
    const tag = found.inline ? "span" : "div";
    const content = found.text
      ? "inline*"
      : found.children
        ? found.inline
          ? "inline*"
          : "block*"
        : "";
    widened.push(
      Node.create({
        name,
        group: found.inline ? "inline" : "block",
        inline: found.inline,
        atom: content === "",
        content,
        addAttributes: () => attributes(found.attributes),
        parseHTML: () => [{ tag: `${tag}[data-nc-unknown="${name}"]` }],
        renderHTML: ({ HTMLAttributes }) =>
          content === ""
            ? [tag, { ...HTMLAttributes, "data-nc-unknown": name }]
            : [tag, { ...HTMLAttributes, "data-nc-unknown": name }, 0],
      }),
    );
  }
  for (const [name, keys] of marks) {
    widened.push(
      Mark.create({
        name,
        addAttributes: () => attributes(keys),
        parseHTML: () => [{ tag: `span[data-nc-mark="${name}"]` }],
        renderHTML: ({ HTMLAttributes }) => [
          "span",
          { ...HTMLAttributes, "data-nc-mark": name },
          0,
        ],
      }),
    );
  }
  if (extra.size > 0) {
    widened.push(
      Extension.create({
        name: "ncKeepUnknownAttributes",
        addGlobalAttributes: () =>
          [...extra].map(([type, keys]) => ({
            types: [type],
            attributes: attributes(keys),
          })),
      }),
    );
  }
  return widened;
};
