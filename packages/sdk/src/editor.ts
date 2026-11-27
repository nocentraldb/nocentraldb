// The public types of a rich-text field's binding (RFC 0013, "An editor:
// the binding"; NOC-461). They import only Yjs's types, so the SDK's API
// report shows them and nothing of the binding's internals.
import type * as Y from "yjs";

/** A rich-text field's vocabulary from the lock (NOC-553): its nodes and marks, without ProseMirror's `doc` and `text`. */
export interface Vocabulary {
  readonly nodes: readonly string[];
  readonly marks: readonly string[];
}

/**
 * Whether the editor shows the whole field and may change it (NOC-538,
 * Q2): `shown`; or `notFullyShown`, with its reason, which the guard
 * reports; or `readOnly`, when the member may only read the field or the
 * binding writes nothing more. A reader's binding whose document the
 * editor does not fully show also reports `readOnly`.
 */
export type RichTextStatus =
  | { readonly status: "shown" }
  | { readonly status: "notFullyShown"; readonly reason: string }
  | { readonly status: "readOnly" };

/** A binding on a rich-text field, which `space.richText` opens. */
export interface RichTextBinding {
  /** The document the editor binds; a reload replaces it (`onReload`). */
  readonly doc: Y.Doc;
  /** Calls `listener` with the new document after a reload; answers a function that stops it. */
  onReload(listener: (doc: Y.Doc) => void): () => void;
  /** The status now. */
  readonly status: RichTextStatus;
  /** Calls `listener` with each new status; answers a function that stops it. */
  onStatus(listener: (status: RichTextStatus) => void): () => void;
  /** The one UndoManager every editor of the binding uses, so a rebuild keeps the undo history. */
  readonly undoManager: Y.UndoManager;
  /** Whether the member may write the field: a writer or an admin. */
  readonly writable: boolean;
  /** The field's vocabulary from the lock: the nodes and marks an editor for it must have (NOC-553). */
  readonly vocabulary: Vocabulary;
  /** Whether an IME composition is open in the editor: the binding holds its writes and pulls meanwhile, for at most 2 s. */
  composing(open: boolean): void;
  /** Writes what is left and gives the client id back. */
  close(): Promise<void>;
}
