// The TipTap test page's script (NOC-461), which tiptap.e2e.test.mjs
// bundles with esbuild: TipTap and @nocentraldb/tiptap from the
// repository, the SDK and Yjs left to the page, so the page, the SDK's
// binding and y-tiptap share one Yjs.
import { nc } from "@nocentraldb/sdk";
import { ncEditor } from "@nocentraldb/tiptap";
import { Editor, getSchema, Node } from "@tiptap/core";
import Collaboration from "@tiptap/extension-collaboration";
import StarterKit from "@tiptap/starter-kit";
import * as Y from "yjs";

window.tt = {
  nc,
  ncEditor,
  Editor,
  getSchema,
  Node,
  Collaboration,
  StarterKit,
  Y,
};
window.ready = true;
