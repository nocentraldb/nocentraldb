// The older app's editor: TipTap with StarterKit and Collaboration on the
// binding's Y.Doc, with the three measures each switchable, and a rebuild
// on contentError (RFC 0013: `body.rebuild(editor)`). `undoManager` given,
// one UndoManager outlives the rebuilds, to see whether the undo history
// has to be lost with them.
import { Editor, getSchema } from "@tiptap/core";
import Collaboration from "@tiptap/extension-collaboration";
import StarterKit from "@tiptap/starter-kit";
import {
  defaultDeleteFilter,
  defaultProtectedNodes,
  ySyncPluginKey,
} from "@tiptap/y-tiptap";
import * as Y from "yjs";
import { widen } from "./measures.js";

const base = () => [StarterKit.configure({ undoRedo: false })];

/** The older app's own schema, without placeholders. */
export const olderSchema = getSchema(base());

export const openEditor = ({
  element,
  binding,
  widening = true,
  contentCheck = true,
  rebuildOnError = true,
  keepUndo = false,
  roundTrip = true,
  errorVia = "option",
  onSchema,
}) => {
  const ed = {
    editor: null,
    contentErrors: 0,
    rebuilds: 0,
    widenedWith: [],
    undoManager: null,
  };
  const build = () => {
    const fragment = binding.doc.getXmlFragment("default");
    const extra = widening ? widen(fragment, olderSchema, { roundTrip }) : null;
    ed.widenedWith = extra
      ? [
          ...[...extra.unknown.nodes.keys()].map((n) => `node ${n}`),
          ...[...extra.unknown.marks.keys()].map((n) => `mark ${n}`),
          ...[...extra.unknown.attrs].map(
            ([n, a]) => `${n}@${[...a].join(",")}`,
          ),
        ]
      : [];
    if (keepUndo && (!ed.undoManager || ed.undoManager.doc !== binding.doc)) {
      ed.undoManager = new Y.UndoManager(fragment, {
        trackedOrigins: new Set([ySyncPluginKey]),
        deleteFilter: (item) =>
          defaultDeleteFilter(item, defaultProtectedNodes),
        captureTransaction: (tr) => tr.meta.get("addToHistory") !== false,
      });
    }
    const own = [...base(), ...(extra?.extensions ?? [])];
    // The guard needs the schema before the editor exists: y-tiptap
    // deletes what the schema lacks while the constructor renders.
    ed.schemaValue = getSchema(own);
    const onError = () => {
      ed.contentErrors += 1;
      if (rebuildOnError) setTimeout(rebuild, 0);
    };
    ed.editor = new Editor({
      element,
      enableContentCheck: contentCheck,
      // TipTap's default onContentError throws, before any listener runs.
      ...(errorVia === "option" ? { onContentError: onError } : {}),
      extensions: [
        ...own,
        Collaboration.configure({
          document: binding.doc,
          ...(ed.undoManager
            ? { yUndoOptions: { undoManager: ed.undoManager } }
            : {}),
        }),
      ],
    });
    // RFC 0013's example listens instead: editor.on("contentError", ...).
    if (errorVia === "listener") ed.editor.on("contentError", onError);
  };
  const rebuild = () => {
    ed.rebuilds += 1;
    ed.editor.destroy();
    element.innerHTML = "";
    build();
  };
  ed.rebuild = rebuild;
  ed.schema = () => ed.schemaValue;
  onSchema?.(ed.schema);
  build();
  binding.on((event) => {
    if (event.type === "reload") rebuild();
  });
  return ed;
};

/** The ProseMirror position just after `text` in the editor, or before it with `before`. */
export const positionOf = (editor, text, { before = false } = {}) => {
  if (text === "") return 1;
  let found = null;
  editor.state.doc.descendants((node, pos) => {
    if (found !== null || !node.isText) return;
    const i = node.text.indexOf(text);
    if (i >= 0) found = pos + i + (before ? 0 : text.length);
  });
  if (found === null)
    throw new Error(`no text ${JSON.stringify(text)} in the editor`);
  return found;
};

/** The position of the first node named `name`. */
export const nodePosition = (editor, name) => {
  let found = null;
  editor.state.doc.descendants((node, pos) => {
    if (found === null && node.type.name === name) found = pos;
  });
  if (found === null) throw new Error(`no node ${name} in the editor`);
  return found;
};
