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
//   While an IME composition is open, it is applied at its end, and after
//   at most MAX_HOLD_MS: TipTap ends a composition itself when it meets a
//   node its schema lacks, before any rebuild can wait for compositionend
//   (NOC-555; Carsten, 2026-11-27, NOC-556).
// - One UndoManager for the document, which every editor of the binding
//   uses (Carsten, 2026-11-22, NOC-467).
import * as Y from "yjs";
import { type Known, undeclared, unknownDeleted } from "./guard.ts";
import type { RichTextBinding, RichTextStatus, Vocabulary } from "./types.ts";

export type { Known } from "./guard.ts";
export type { RichTextBinding, RichTextStatus, Vocabulary } from "./types.ts";

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

/** The binding as the SDK holds it: the public one, and what the SDK itself calls. */
export interface Binding extends RichTextBinding {
  /** Pulls what the core folded since; call it after a fold of the field's object. While a composition is open, the pull waits for its end, for at most 2 s. */
  folded(): Promise<void>;
  /** Writes at once: the page is hidden. */
  hidden(): Promise<void>;
}

/** The origin under which the binding applies what the core folded. */
const FROM_CORE = Symbol("nocentraldb: from the core");

/** What the binding is opened with besides the core's answer. */
export interface BindOptions {
  /** The names the editor knows, which the guard checks deletions against; without it, the guard is off. */
  readonly known?: () => Known;
  /** The field's vocabulary from the lock; a document with other names is reported as not fully shown. */
  readonly vocabulary?: Vocabulary;
}

/** Opens the binding's document on what the core answered. `refused` hears of a write the core refused; the binding then writes nothing more, its status is `readOnly`, and it loads its document again from the core (`onReload`). */
export const bind = (
  opened: Opened,
  port: RichTextPort,
  refused: (error: unknown) => void = () => {},
  options: BindOptions = {},
): Binding => {
  let doc: Y.Doc;
  let undoManager: Y.UndoManager;
  // Unknown names a local edit not yet written deleted (the guard).
  let deletes: string[] = [];
  let trips = 0;
  const reloadListeners = new Set<(doc: Y.Doc) => void>();

  /** A document under the leased client id, loaded from `state`. */
  const load = (state: Uint8Array) => {
    const next = new Y.Doc();
    const root = next.getXmlFragment(ROOT);
    // The state may hold items of the leased client id, written before
    // under the same lease. Applied under that id, Yjs would draw a new one
    // ("another client seems to be using it"), so the id is set after.
    Y.applyUpdate(next, state, FROM_CORE);
    next.clientID = opened.client;
    // It tracks the edits made without an origin; an editor adds its own
    // (TipTap's sync plugin key, with @nocentraldb/tiptap). It never tracks
    // FROM_CORE, so what another member wrote is never taken back.
    const origins = undoManager?.trackedOrigins ?? new Set([null]);
    undoManager = new Y.UndoManager(root, { trackedOrigins: new Set(origins) });
    root.observeDeep((events, tr) => {
      if (next !== doc || tr.origin === FROM_CORE || !options.known) return;
      const names = unknownDeleted(events, options.known());
      deletes = [...new Set([...deletes, ...names])];
    });
    next.on("update", (update: Uint8Array, origin: unknown) => {
      if (next !== doc) return;
      edited(update, origin);
    });
    doc = next;
  };

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

  // Pulls asked for while a composition is open, run as one at its end.
  let waiting: { resolve: () => void; reject: (e: unknown) => void }[] = [];
  let pullCap: ReturnType<typeof setTimeout> | undefined;
  /** Reports the document as not fully shown while it holds names the vocabulary does not declare. */
  const checkVocabulary = () => {
    if (!options.vocabulary || status.status !== "shown") return;
    const names = undeclared(doc.getXmlFragment(ROOT), options.vocabulary);
    if (names.length === 0) return;
    report({
      status: "notFullyShown",
      reason: `the content uses names this version of the app does not declare: ${names.join(", ")}`,
    });
  };

  const pull = async () => {
    const missing = await port.pull(Y.encodeStateVector(doc));
    Y.applyUpdate(doc, missing, FROM_CORE);
    checkVocabulary();
  };
  const pullWaiting = () => {
    clearTimeout(pullCap);
    pullCap = undefined;
    const now = waiting;
    waiting = [];
    if (now.length === 0) return;
    pull().then(
      () => {
        for (const w of now) w.resolve();
      },
      (error) => {
        for (const w of now) w.reject(error);
      },
    );
  };

  const clearTimers = () => {
    clearTimeout(pause);
    clearTimeout(hold);
    pause = hold = undefined;
  };

  /** Loads the document again from the core's state and tells onReload. */
  const reload = async () => {
    load(await port.pull(Y.encodeStateVector(new Y.Doc())));
    for (const listener of [...reloadListeners]) {
      try {
        listener(doc);
      } catch {
        // A listener that throws misses nothing the others hear.
      }
    }
  };

  /** The guard refused the edits not yet written: reload once, then stop. */
  const refuse = async () => {
    const names = deletes;
    deletes = [];
    unwritten = [];
    trips += 1;
    if (trips > 1) {
      stopped = true;
      report({ status: "readOnly" });
      return;
    }
    await reload();
    report({
      status: "notFullyShown",
      reason: `the editor would delete parts it does not know: ${names.join(", ")}`,
    });
  };

  const write = (): Promise<void> => {
    clearTimers();
    if (!stopped && deletes.length > 0) {
      writing = writing.then(refuse);
      return writing;
    }
    if (stopped || unwritten.length === 0) return writing;
    const update = Y.mergeUpdates(unwritten);
    unwritten = [];
    writing = writing.then(async () => {
      if (stopped) return;
      try {
        await port.write(update);
      } catch (error) {
        stopped = true;
        unwritten = [];
        report({ status: "readOnly" });
        refused(error);
        // The editor must not show text no change holds (RFC 0013, "A
        // reader"): the document is loaded again from the core.
        await reload();
      }
    });
    return writing;
  };

  function edited(update: Uint8Array, origin: unknown) {
    if (origin === FROM_CORE || stopped) return;
    unwritten.push(update);
    clearTimeout(pause);
    pause = setTimeout(() => {
      pause = undefined;
      if (!composing) void write();
    }, PAUSE_MS);
    hold ??= setTimeout(() => void write(), MAX_HOLD_MS);
  }

  load(opened.state);
  checkVocabulary();

  return {
    get doc() {
      return doc;
    },
    get undoManager() {
      return undoManager;
    },
    onReload: (listener) => {
      reloadListeners.add(listener);
      return () => reloadListeners.delete(listener);
    },
    writable: opened.writable,
    vocabulary: options.vocabulary ?? { nodes: [], marks: [] },
    get status() {
      return status;
    },
    onStatus: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    folded: () => {
      if (!composing) return pull();
      return new Promise<void>((resolve, reject) => {
        waiting.push({ resolve, reject });
        pullCap ??= setTimeout(pullWaiting, MAX_HOLD_MS);
      });
    },
    composing: (open) => {
      composing = open;
      if (!open) pullWaiting();
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
