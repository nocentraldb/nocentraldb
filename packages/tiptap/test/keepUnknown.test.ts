// An app that builds its own editor with NcKeepUnknown and leaves out the
// content check is refused as the editor is created, with
// `contentCheckMissing` (RFC 0013; Carsten, 2026-11-22, NOC-467;
// NOC-538, Q3).
import { Editor } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import { expect, test } from "vitest";
import * as Y from "yjs";
import { NcKeepUnknown } from "../src/keepUnknown.ts";

test("an editor without enableContentCheck is refused with contentCheckMissing", () => {
  const doc = new Y.Doc();
  expect(
    () =>
      new Editor({
        extensions: [
          StarterKit.configure({ undoRedo: false }),
          NcKeepUnknown.configure({ document: doc }),
        ],
      }),
  ).toThrowError(expect.objectContaining({ code: "contentCheckMissing" }));
});

test("with the content check, it is accepted", () => {
  const doc = new Y.Doc();
  const editor = new Editor({
    enableContentCheck: true,
    extensions: [
      StarterKit.configure({ undoRedo: false }),
      NcKeepUnknown.configure({ document: doc }),
    ],
  });
  expect(editor.isDestroyed).toBe(false);
  editor.destroy();
});
