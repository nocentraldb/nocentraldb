// `ncEditor(body, options)`: the TipTap editor of a rich-text field (RFC
// 0013, "An editor that cannot show a node"; Carsten, 2026-11-27, NOC-538
// Q4, b, as the spike on NOC-555 confirmed). It builds the editor itself:
//
// - with `enableContentCheck`, the binding's Y.Doc and its one UndoManager,
//   to which it adds y-tiptap's sync origin, so a rebuild keeps the undo
//   history (NOC-467);
// - with a schema widened from the document, whose placeholders it tells
//   the binding through `known`, so the guard lets their deliberate
//   deletion through;
// - after checking the editor against the field's vocabulary, which
//   throws `vocabularyMismatch` (NOC-553).
//
// It rebuilds the editor when a new kind of node arrives (contentError) and
// when the binding reloads its document, restores the selection, kept as
// Yjs relative positions, and the focus, noted as a remote transaction
// starts, since TipTap takes the focus away before onContentError (NOC-555),
// and hands each editor to `onEditor`. A composition in the editor reaches
// the binding, which holds its writes and pulls meanwhile.

import type { KnownNames, RichTextBinding } from "@nocentraldb/sdk";
import {
  type AnyExtension,
  Editor,
  type EditorOptions,
  Extension,
  getSchema,
} from "@tiptap/core";
import Collaboration from "@tiptap/extension-collaboration";
import { type EditorState, Plugin, TextSelection } from "@tiptap/pm/state";
import {
  absolutePositionToRelativePosition,
  relativePositionToAbsolutePosition,
  ySyncPluginKey,
} from "@tiptap/y-tiptap";
import type * as Y from "yjs";
import { checkVocabulary } from "./vocabulary.ts";
import { widen } from "./widen.ts";

export interface NcEditorOptions {
  /** Where the editor mounts. */
  element: HTMLElement;
  /** The editor's extensions, such as StarterKit, without Collaboration and its undo, which ncEditor adds. */
  extensions: AnyExtension[];
  /** Any other options of TipTap's editor; ncEditor sets its own content check, element, extensions and editable. */
  editorOptions?: Partial<EditorOptions>;
  /** Called with the first editor and with each one a rebuild makes; read the editor from here, never from a closure. */
  onEditor: (editor: Editor) => void;
}

/**
 * Rendering never writes (NOC-557): a transaction a plugin appends to y-sync's
 * render of the document, such as Link's autolink on text that looks like a
 * URL or the trailing paragraph, is refused, since it would reach the Y.Doc
 * as an edit nobody made. ProseMirror filters an appended transaction
 * before it marks it as appended, so the filter notes the render it lets
 * through and refuses every transaction that changes the document until
 * the view has taken the render in; an edit after that, also one an app
 * makes in the same task, goes through. The selection a rebuild restores is
 * treated as a render: what a plugin appends to it, such as a trailing
 * paragraph, is not written either.
 */
/** Marks the transaction that restores the selection after a rebuild, which, like a render, nobody made. */
const RESTORE = "ncRestoreSelection";

/**
 * A first edit that empties the document is written (NOC-580). y-tiptap
 * 3.3.1 writes a change only once the document has differed from an empty
 * one, and its own first render, dispatched before its view's update hook
 * exists, does not count: an edit that leaves the document empty before any
 * other was never written. When an edit, not a render, leaves the document
 * empty, it is handed to y-sync's binding as y-sync's own update hook hands
 * it: inside the binding's mutex, in a transaction under y-sync's origin,
 * so the binding's UndoManager tracks it. If y-sync wrote it already, the
 * Y.Doc holds it and nothing more is written. y-tiptap is pinned exactly,
 * since this reads its binding.
 */
const writeEmptied = (state: EditorState) => {
  const empty = state.doc.type.createAndFill();
  if (!empty || !state.doc.eq(empty)) return;
  const binding = ySyncPluginKey.getState(state)?.binding;
  if (!binding) return;
  binding.mux(() => {
    binding.doc.transact((tr) => {
      tr.meta.set("addToHistory", true);
      binding._prosemirrorChanged(state.doc);
    }, ySyncPluginKey);
  });
};

const noWriteOnRender = () => {
  let rendering = false;
  return Extension.create({
    name: "ncNoWriteOnRender",
    // Before y-sync's plugin, so its view exists when y-sync renders the
    // document while the editor's view is built.
    priority: 1000,
    addProseMirrorPlugins: () => [
      new Plugin({
        filterTransaction: (tr) => {
          if (
            tr.getMeta(ySyncPluginKey)?.isChangeOrigin ||
            tr.getMeta(RESTORE)
          ) {
            rendering = true;
            // At the latest, the task's end ends the render.
            queueMicrotask(() => {
              rendering = false;
            });
            return true;
          }
          return !(rendering && tr.docChanged);
        },
        view: () => ({
          update: (view, previous) => {
            const rendered = rendering;
            // The view takes the render, and what was appended to it, in at
            // once: from then on, transactions are no longer appended to it.
            rendering = false;
            if (!rendered && !view.state.doc.eq(previous.doc)) {
              writeEmptied(view.state);
            }
          },
        }),
      }),
    ],
  });
};

/** The names `widened` adds to the schema `base` builds. */
const placeholderNames = (
  base: AnyExtension[],
  widened: AnyExtension[],
): KnownNames => {
  const before = getSchema(base);
  const after = getSchema([...base, ...widened]);
  const attributes: Record<string, string[]> = {};
  for (const [name, type] of Object.entries(after.nodes)) {
    const old = before.nodes[name]?.spec.attrs ?? {};
    const added = Object.keys(type.spec.attrs ?? {}).filter((a) => !(a in old));
    if (added.length > 0) attributes[name] = added;
  }
  return {
    nodes: Object.keys(after.nodes).filter((n) => !before.nodes[n]),
    marks: Object.keys(after.marks).filter((m) => !before.marks[m]),
    attributes,
  };
};

/** Builds the field's editor and keeps rebuilding it; `destroy` ends it. */
export const ncEditor = (
  body: RichTextBinding,
  options: NcEditorOptions,
): { destroy(): void } => {
  const { element, extensions, editorOptions, onEditor } = options;
  checkVocabulary(extensions, body.vocabulary);

  let editor: Editor | undefined;
  let composing = false;
  let pending = false;
  let hadFocus = false;
  let stopDoc = () => {};

  const editable = () => body.writable && body.status.status !== "readOnly";

  const watch = (doc: Y.Doc) => {
    stopDoc();
    // TipTap takes the focus away before onContentError: it is noted as a
    // remote update starts, before the editor sees it (NOC-555).
    const before = (tr: { local: boolean }) => {
      if (!tr.local && !pending) {
        hadFocus = element.contains(document.activeElement);
      }
    };
    doc.on("beforeTransaction", before);
    stopDoc = () => doc.off("beforeTransaction", before);
  };

  const create = (
    selection?: { anchor: unknown; head: unknown },
    focus = false,
  ) => {
    body.undoManager.addTrackedOrigin(ySyncPluginKey);
    const widened = widen(body.doc, extensions);
    body.known(placeholderNames(extensions, widened));
    watch(body.doc);
    editor = new Editor({
      ...editorOptions,
      element,
      editable: editable(),
      enableContentCheck: true,
      onContentError: () => {
        pending = true;
        if (!composing) setTimeout(rebuild, 0);
      },
      extensions: [
        ...extensions,
        ...widened,
        Collaboration.configure({
          document: body.doc,
          yUndoOptions: { undoManager: body.undoManager },
        }),
        noWriteOnRender(),
      ],
    });
    const sync = ySyncPluginKey.getState(editor.state);
    if (selection && sync) {
      const anchor = relativePositionToAbsolutePosition(
        body.doc,
        sync.type,
        selection.anchor as never,
        sync.binding.mapping,
      );
      const head = relativePositionToAbsolutePosition(
        body.doc,
        sync.type,
        selection.head as never,
        sync.binding.mapping,
      );
      if (anchor !== null && head !== null) {
        const { state } = editor;
        const max = state.doc.content.size;
        editor.view.dispatch(
          state.tr
            .setSelection(
              TextSelection.create(
                state.doc,
                Math.min(anchor, max),
                Math.min(head, max),
              ),
            )
            .setMeta(RESTORE, true),
        );
      }
    }
    if (focus) editor.view.focus();
    onEditor(editor);
  };

  /** Builds the editor again; `keepSelection` when the document stays the same. */
  const rebuild = (keepSelection = true) => {
    if (!editor || (keepSelection && (!pending || composing))) return;
    pending = false;
    let selection: { anchor: unknown; head: unknown } | undefined;
    const sync = ySyncPluginKey.getState(editor.state);
    if (keepSelection && sync) {
      const { anchor, head } = editor.state.selection;
      selection = {
        anchor: absolutePositionToRelativePosition(
          anchor,
          sync.type,
          sync.binding.mapping,
        ),
        head: absolutePositionToRelativePosition(
          head,
          sync.type,
          sync.binding.mapping,
        ),
      };
    }
    const focus =
      hadFocus || editor.isFocused || element.contains(document.activeElement);
    hadFocus = false;
    editor.destroy();
    create(selection, focus);
  };

  const compositionStart = () => {
    composing = true;
    body.composing(true);
  };
  const compositionEnd = () => {
    composing = false;
    body.composing(false);
    if (pending) setTimeout(rebuild, 0);
  };
  element.addEventListener("compositionstart", compositionStart, true);
  element.addEventListener("compositionend", compositionEnd, true);
  const stopReload = body.onReload(() => rebuild(false));
  const stopStatus = body.onStatus(() => editor?.setEditable(editable()));

  create();
  return {
    destroy: () => {
      element.removeEventListener("compositionstart", compositionStart, true);
      element.removeEventListener("compositionend", compositionEnd, true);
      stopReload();
      stopStatus();
      stopDoc();
      editor?.destroy();
    },
  };
};
