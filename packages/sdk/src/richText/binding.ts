// The rich-text editor binding's Y.Doc (RFC 0013, "An editor: the
// binding"; NOC-461): a Yjs document under the client id the core leased,
// loaded from the field's state, which the binding keeps in step with the
// core. Internal: the public call that opens it comes with the owner's
// answers on NOC-538.
//
// - The editor's edits are written as one change at each pause in typing:
//   after PAUSE_MS without an edit, after at most MAX_HOLD_MS of typing,
//   and when the page is hidden or the binding closes (Carsten,
//   2026-11-21, NOC-430).
// - While an IME composition is open, the write waits for its end, and for
//   at most MAX_HOLD_MS since the first edit not yet written (Carsten,
//   2026-11-22, NOC-467).
// - What the core folded is applied under the binding's own origin, which
//   the binding never writes back and its UndoManager never takes back.
// - One UndoManager for the document, which every editor of the binding
//   uses (Carsten, 2026-11-22, NOC-467).
import * as Y from "yjs";

/** The pause in typing after which the binding writes (RFC 0013, "An editor: the binding"). */
export const PAUSE_MS = 500;
/** The longest an edit waits to be written: while typing goes on, or while an IME composition is open. */
export const MAX_HOLD_MS = 2000;

/** The root fragment TipTap's collaboration binds (RFC 0013). */
const ROOT = "default";

/** What the binding asks of the core, through the bridge. */
export interface RichTextPort {
  /** The part of the field's document a document with this state vector lacks. */
  pull(stateVector: Uint8Array): Promise<Uint8Array>;
  /** Writes one Yjs update as a change; throws when the core refuses it. */
  write(update: Uint8Array): Promise<void>;
  /** Gives the leased client id back. */
  close(): Promise<void>;
}

/** What the core answered when the binding opened. */
export interface Opened {
  readonly client: number;
  readonly state: Uint8Array;
  readonly writable: boolean;
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

export interface Binding {
  readonly doc: Y.Doc;
  /** The status now. */
  readonly status: RichTextStatus;
  /** Calls `listener` with each new status; answers a function that stops it. */
  onStatus(listener: (status: RichTextStatus) => void): () => void;
  readonly undoManager: Y.UndoManager;
  readonly writable: boolean;
  /** Pulls what the core folded since; call it after a fold of the field's collection. */
  folded(): Promise<void>;
  /** Whether an IME composition is open in the editor. */
  composing(open: boolean): void;
  /** Writes at once: the page is hidden. */
  hidden(): Promise<void>;
  /** Writes what is left and gives the client id back. */
  close(): Promise<void>;
}

/** The origin under which the binding applies what the core folded. */
const FROM_CORE = Symbol("nocentraldb: from the core");

/** Opens the binding's document on what the core answered. `refused` hears of a write the core refused; the binding then writes nothing more and its status is `readOnly`. */
export const bind = (
  opened: Opened,
  port: RichTextPort,
  refused: (error: unknown) => void = () => {},
): Binding => {
  const doc = new Y.Doc();
  doc.clientID = opened.client;
  const root = doc.getXmlFragment(ROOT);
  Y.applyUpdate(doc, opened.state, FROM_CORE);
  // It tracks the edits made without an origin; an editor adds its own
  // (TipTap's sync plugin key, with @nocentraldb/tiptap). It never tracks
  // FROM_CORE, so what another member wrote is never taken back.
  const undoManager = new Y.UndoManager(root, {
    trackedOrigins: new Set([null]),
  });

  let unwritten: Uint8Array[] = [];
  let pause: ReturnType<typeof setTimeout> | undefined;
  let hold: ReturnType<typeof setTimeout> | undefined;
  let composing = false;
  let stopped = !opened.writable;
  // Writes go one at a time, in order.
  let writing: Promise<void> = Promise.resolve();
  let status: RichTextStatus = opened.writable
    ? { status: "shown" }
    : { status: "readOnly" };
  const listeners = new Set<(status: RichTextStatus) => void>();
  const report = (next: RichTextStatus) => {
    status = next;
    for (const listener of [...listeners]) {
      try {
        listener(next);
      } catch {
        // A listener that throws misses nothing the others hear.
      }
    }
  };

  const clearTimers = () => {
    clearTimeout(pause);
    clearTimeout(hold);
    pause = hold = undefined;
  };

  const write = (): Promise<void> => {
    clearTimers();
    if (stopped || unwritten.length === 0) return writing;
    const update = Y.mergeUpdates(unwritten);
    unwritten = [];
    writing = writing.then(async () => {
      if (stopped) return;
      try {
        await port.write(update);
      } catch (error) {
        stopped = true;
        report({ status: "readOnly" });
        refused(error);
      }
    });
    return writing;
  };

  doc.on("update", (update: Uint8Array, origin: unknown) => {
    if (origin === FROM_CORE || stopped) return;
    unwritten.push(update);
    clearTimeout(pause);
    pause = setTimeout(() => {
      pause = undefined;
      if (!composing) void write();
    }, PAUSE_MS);
    hold ??= setTimeout(() => void write(), MAX_HOLD_MS);
  });

  return {
    doc,
    undoManager,
    writable: opened.writable,
    get status() {
      return status;
    },
    onStatus: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    folded: async () => {
      const missing = await port.pull(Y.encodeStateVector(doc));
      Y.applyUpdate(doc, missing, FROM_CORE);
    },
    composing: (open) => {
      composing = open;
      if (!open && unwritten.length > 0 && pause === undefined) void write();
    },
    hidden: () => write(),
    close: async () => {
      await write();
      stopped = true;
      undoManager.destroy();
      await port.close();
    },
  };
};
