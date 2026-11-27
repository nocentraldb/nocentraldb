// The guard on what the binding writes (RFC 0013, "An editor that cannot
// show a node", measure 3): which parts the editor does not know a local
// transaction deletes. It reads the transaction's events, before Yjs
// collects the deleted content:
//
// - an element whose name the editor does not know, deleted, or one inside
//   a deleted element;
// - a format the editor does not know, taken off text that stays (a
//   format that goes with its text goes with a deletion the editor knows);
// - an attribute the editor does not declare for its node, removed.
import * as Y from "yjs";

/** The names the editor knows: the field's vocabulary and what the editor declares besides, its placeholders included. */
export interface Known {
  readonly nodes: readonly string[];
  readonly marks: readonly string[];
  /** The attributes the editor declares, by node; a node not listed is not checked. */
  readonly attributes?: Readonly<Record<string, readonly string[]>>;
}

/** The names of unknown parts the events delete, each once, in the order met. */
export const unknownDeleted = (
  events: readonly Y.YEvent<Y.AbstractType<unknown>>[],
  known: Known,
): string[] => {
  const nodes = new Set(known.nodes);
  const marks = new Set(known.marks);
  const found = new Set<string>();
  const element = (type: Y.XmlElement) => {
    if (!nodes.has(type.nodeName)) found.add(type.nodeName);
    for (const child of type.toArray()) {
      if (child instanceof Y.XmlElement) element(child);
    }
  };
  for (const event of events) {
    for (const item of event.changes.deleted) {
      const content = item.content;
      if (
        content instanceof Y.ContentType &&
        content.type instanceof Y.XmlElement
      ) {
        element(content.type);
      }
    }
    if (event.target instanceof Y.XmlText) {
      for (const op of event.delta) {
        if (op.retain === undefined || !op.attributes) continue;
        for (const [mark, value] of Object.entries(op.attributes)) {
          if (value === null && !marks.has(mark)) found.add(mark);
        }
      }
    }
    if (event.target instanceof Y.XmlElement) {
      const declared = known.attributes?.[event.target.nodeName];
      if (!declared) continue;
      for (const [key, change] of event.changes.keys) {
        if (change.action === "delete" && !declared.includes(key)) {
          found.add(`${event.target.nodeName}.${key}`);
        }
      }
    }
  }
  return [...found];
};
