// The editors the spike binds to a Yjs document: a bare ProseMirror view with
// y-prosemirror's or y-tiptap's sync plugin, and a TipTap editor with its
// Collaboration extension, with and without enableContentCheck.
import { Editor } from "@tiptap/core";
import Collaboration from "@tiptap/extension-collaboration";
import StarterKit from "@tiptap/starter-kit";
import * as YT from "@tiptap/y-tiptap";
import { EditorState } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import * as YP from "y-prosemirror";

const mount = () => document.body.appendChild(document.createElement("div"));

const viewEditor = (plugin, schema, props = {}) => {
  const view = new EditorView(mount(), {
    ...props,
    state: EditorState.create({ schema, plugins: [plugin] }),
  });
  return { view, destroy: () => view.destroy() };
};

export const yProsemirror = (doc, schema, props) =>
  viewEditor(YP.ySyncPlugin(doc.getXmlFragment("default")), schema, props);

export const yTiptap = (doc, schema, { onInvalidContent } = {}) =>
  viewEditor(
    YT.ySyncPlugin(doc.getXmlFragment("default"), { onInvalidContent }),
    schema,
  );

// The older app as TipTap: StarterKit, whose names include paragraph, text and
// bold, so it knows exactly what oldSchema knows and more of StarterKit.
export const tiptap = (doc, { enableContentCheck, extensions = [] } = {}) => {
  const errors = [];
  const editor = new Editor({
    element: mount(),
    enableContentCheck,
    onContentError: ({ error }) => errors.push(error.message),
    extensions: [
      StarterKit.configure({ undoRedo: false }),
      Collaboration.configure({ document: doc }),
      ...extensions,
    ],
  });
  return { view: editor.view, editor, errors, destroy: () => editor.destroy() };
};

// Types text at the start of the first text block whose text starts with
// `prefix`, the way a keystroke would: one ProseMirror transaction.
export const typeAt = (view, prefix, text) => {
  let pos = null;
  view.state.doc.descendants((node, p) => {
    if (pos !== null) return false;
    if (node.isTextblock && node.textContent.startsWith(prefix)) pos = p + 1;
  });
  if (pos === null) return { typed: false, reason: "block not shown" };
  const before = view.state.doc;
  view.dispatch(view.state.tr.insertText(text, pos));
  return {
    typed: !view.state.doc.eq(before),
    reason: view.state.doc.eq(before) ? "transaction filtered" : null,
  };
};

export const shown = (view) =>
  view.state.doc.content.content.map((n) => n.textContent);
