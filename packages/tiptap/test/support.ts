// A binding as `space.richText` gives one, over a Y.Doc of the test's own,
// recording what the editor layer tells it.
import type {
  KnownNames,
  RichTextBinding,
  RichTextStatus,
  Vocabulary,
} from "@nocentraldb/sdk";
import * as Y from "yjs";

export interface FakeBinding extends RichTextBinding {
  /** Every value `composing` was called with. */
  readonly compositions: boolean[];
  /** The names the editor declared last. */
  declared: KnownNames | undefined;
  /** Replaces the document, as a reload does. */
  reload(doc: Y.Doc): void;
  /** Sets the status, as the binding does. */
  setStatus(status: RichTextStatus): void;
}

export const fakeBinding = (
  vocabulary: Vocabulary,
  build?: (root: Y.XmlFragment) => void,
): FakeBinding => {
  let doc = new Y.Doc();
  build?.(doc.getXmlFragment("default"));
  let undoManager = new Y.UndoManager(doc.getXmlFragment("default"));
  let status: RichTextStatus = { status: "shown" };
  const reloads = new Set<(doc: Y.Doc) => void>();
  const statuses = new Set<(status: RichTextStatus) => void>();
  const binding: FakeBinding = {
    get doc() {
      return doc;
    },
    get undoManager() {
      return undoManager;
    },
    get status() {
      return status;
    },
    writable: true,
    vocabulary,
    compositions: [],
    declared: undefined,
    onReload: (listener) => {
      reloads.add(listener);
      return () => reloads.delete(listener);
    },
    onStatus: (listener) => {
      statuses.add(listener);
      return () => statuses.delete(listener);
    },
    composing: (open) => binding.compositions.push(open),
    known: (names) => {
      binding.declared = names;
    },
    close: async () => {},
    reload: (next) => {
      doc = next;
      undoManager = new Y.UndoManager(next.getXmlFragment("default"));
      for (const listener of reloads) listener(next);
    },
    setStatus: (next) => {
      status = next;
      for (const listener of statuses) listener(next);
    },
  };
  return binding;
};

/** A paragraph of `words`. */
export const paragraph = (words: string) => {
  const p = new Y.XmlElement("paragraph");
  p.insert(0, [new Y.XmlText(words)]);
  return p;
};
