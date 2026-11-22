// The page the spike's runner drives: window.spike sets up a binding and an
// editor on a core, and reports what the binding wrote, what the core holds,
// and what the older app lost.

import { NodeSelection, TextSelection } from "@tiptap/pm/state";
import * as Y from "yjs";
import { Binding, FROM_CORE } from "./binding.js";
import { pageCore } from "./core.js";
import { nodePosition, openEditor, positionOf } from "./editor.js";
import { guardFor } from "./measures.js";
import { lost, newerApp, xml } from "./newer.js";
import { sdkCore } from "./sdk-core.js";

const S = {};
const now = () => performance.timeOrigin + performance.now();

const mount = () => {
  const element = document.getElementById("editor");
  element.innerHTML = "";
  return element;
};

const bindAndEdit = async (core, o) => {
  const ref = {};
  const binding = await new Binding({
    core,
    clientID: o.clientID ?? 2,
    ...(o.idleMs !== undefined ? { idleMs: o.idleMs, maxMs: o.maxMs } : {}),
    filterOwnOrigin: o.filterOwnOrigin ?? true,
    guard: o.guard === false ? null : guardFor(() => ref.schema?.()),
  }).open();
  ref.ed =
    o.editor === false
      ? null
      : openEditor({
          element: mount(),
          binding,
          ...o,
          onSchema: (f) => (ref.schema = f),
        });
  return { binding, ed: ref.ed };
};

window.spike = {
  Y,
  FROM_CORE,

  /** A page core, the newer app's document in it when `newerDocument`, and the older app's binding and editor. */
  async setupLocal(o = {}) {
    S.core = pageCore();
    S.newer = newerApp(S.core);
    if (o.newerDocument) await S.newer.writeDocument();
    else await S.core.write(new Uint8Array([0, 0]), { clientID: 0 });
    Object.assign(S, await bindAndEdit(S.core, o));
    // A second member in the same page, without an editor, for the echo.
    if (o.collaborator) {
      S.other = await new Binding({ core: S.core, clientID: 3 }).open();
    }
    return this.report();
  },

  /** A core over the SDK's `db`, on `spaceId`, and a binding and editor on it. */
  async setupSdk({ spaceId, clientID, editor = true, idleMs, maxMs }) {
    S.core = await sdkCore({
      db: window.tab.db,
      model: window.tab.model,
      spaceId,
    });
    Object.assign(
      S,
      await bindAndEdit(S.core, {
        clientID,
        editor,
        guard: false,
        idleMs,
        maxMs,
      }),
    );
    // Same-origin pages read which pause started the last write, even after this tab is gone.
    S.binding.onFlushStart = (reason) =>
      localStorage.setItem("spike.flushStart", reason);
    return true;
  },

  report() {
    const { core, binding, ed } = S;
    return {
      core: xml(core.doc),
      binding: xml(binding.doc),
      lostInCore: lost(core.doc),
      lostInBinding: lost(binding.doc),
      editorText: ed?.editor.getText() ?? null,
      editable: ed?.editor.isEditable ?? null,
      contentErrors: ed?.contentErrors ?? 0,
      rebuilds: ed?.rebuilds ?? 0,
      widenedWith: ed?.widenedWith ?? [],
      undoStack: ed?.editor ? undoDepth(ed) : null,
      stats: {
        ...binding.stats,
        writes: binding.stats.writes.map(
          ({ reason, parts, bytes, clients }) => ({
            reason,
            parts,
            bytes,
            clients,
          }),
        ),
        pulls: binding.stats.pulls.map(({ bytes, changed }) => ({
          bytes,
          changed,
        })),
      },
      coreWrites:
        core.writes?.map(({ clientID, bytes }) => ({ clientID, bytes })) ??
        null,
    };
  },

  /** Puts the cursor after (or before) `text` and focuses the editor. */
  place(text, { before = false } = {}) {
    const { editor } = S.ed;
    const pos = positionOf(editor, text, { before });
    editor.view.dispatch(
      editor.state.tr.setSelection(TextSelection.create(editor.state.doc, pos)),
    );
    editor.view.focus();
  },

  /** Selects from before `from` to after `to`. */
  selectBetween(from, to) {
    const { editor } = S.ed;
    const a = positionOf(editor, from, { before: true });
    const b = positionOf(editor, to);
    editor.view.dispatch(
      editor.state.tr.setSelection(
        TextSelection.create(editor.state.doc, a, b),
      ),
    );
    editor.view.focus();
  },

  /** Selects the first node named `name`. */
  selectNode(name) {
    const { editor } = S.ed;
    const pos = nodePosition(editor, name);
    editor.view.dispatch(
      editor.state.tr.setSelection(NodeSelection.create(editor.state.doc, pos)),
    );
    editor.view.focus();
  },

  /** What the editor puts on the clipboard for its selection, through a copy event. */
  copySelection() {
    const data = new DataTransfer();
    S.ed.editor.view.dom.dispatchEvent(
      new ClipboardEvent("copy", {
        clipboardData: data,
        bubbles: true,
        cancelable: true,
      }),
    );
    return {
      html: data.getData("text/html"),
      text: data.getData("text/plain"),
    };
  },

  /** Pastes `html` and `text` at the selection, through a paste event. */
  paste({ html = "", text = "" }) {
    const data = new DataTransfer();
    if (html) data.setData("text/html", html);
    if (text) data.setData("text/plain", text);
    S.ed.editor.view.dom.dispatchEvent(
      new ClipboardEvent("paste", {
        clipboardData: data,
        bubbles: true,
        cancelable: true,
      }),
    );
  },

  /** The page as hidden, as when the person switches tabs. */
  hide() {
    Object.defineProperty(document, "visibilityState", {
      value: "hidden",
      configurable: true,
    });
    document.dispatchEvent(new Event("visibilitychange"));
  },

  addFigure: () => S.newer.addFigure(),
  undo: () => S.ed.editor.commands.undo(),
  flush: () => S.binding.flush("test"),

  /** The collaborator types `text` at the end of the first paragraph and writes at once. */
  async collaboratorTypes(text) {
    const first = S.other.doc.getXmlFragment("default").get(0);
    const t = first.get(0);
    S.other.doc.transact(() => t.insert(t.length, text));
    await S.other.flush("test");
  },

  /** Resolves with the time the binding's document first holds `token`. */
  whenHolds(token) {
    return new Promise((resolve) => {
      const check = () => {
        if (xml(S.binding.doc).includes(token)) {
          S.binding.doc.off("update", check);
          resolve(now());
          return true;
        }
        return false;
      };
      if (!check()) S.binding.doc.on("update", check);
    });
  },

  editor: () => S.ed,

  /** Puts the cursor at the end of the document. */
  focusEnd: () => S.ed.editor.commands.focus("end"),

  /** From now on, logs when the binding's text grows: [time, length]. */
  logLengths() {
    S.lengths = [];
    const log = () =>
      S.lengths.push([
        now(),
        xml(S.binding.doc).replace(/<[^>]+>/g, "").length,
      ]);
    log();
    S.binding.doc.on("update", log);
  },
  lengths: () => S.lengths,
  textLength: () => xml(S.binding.doc).replace(/<[^>]+>/g, "").length,
  keyTimes: () => S.keyTimes,
  resetKeys: () => {
    S.keyTimes = [];
  },
  writes: () => S.binding.stats.writes,
  lastKeyAt: () => S.lastKeyAt,
  arrivals: () => S.core.arrivals ?? [],
  now,
};

const undoDepth = (ed) => {
  const um =
    ed.undoManager ??
    ed.editor.state.plugins
      .map((p) => p.getState?.(ed.editor.state))
      .find((s) => s?.undoManager)?.undoManager;
  return um ? um.undoStack.length : null;
};

// The time of the last key the person pressed, as the event stamps it.
document.addEventListener(
  "keydown",
  (e) => {
    S.lastKeyAt = performance.timeOrigin + e.timeStamp;
    if (e.key.length !== 1) return;
    S.keyTimes ??= [];
    S.keyTimes.push(S.lastKeyAt);
  },
  true,
);

window.spikeReady = true;
