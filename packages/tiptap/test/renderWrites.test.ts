// Rendering never writes (NOC-557): an editor that opens or rebuilds on a
// document applies plugins' appended transactions, such as Link's autolink
// turning URL-like text into a link, to what it renders. Those would reach
// the Y.Doc as the person's edit and be written, though nobody typed. The
// spike on NOC-555 saw a rebuild do so with "line.abc" at the end of a
// paragraph. ncEditor refuses any transaction appended to a render of the
// document.
import StarterKit from "@tiptap/starter-kit";
import { afterEach, expect, test } from "vitest";
import * as Y from "yjs";
import { ncEditor } from "../src/ncEditor.ts";
import { editorVocabulary } from "../src/vocabulary.ts";
import { fakeBinding, paragraph } from "./support.ts";

const kit = [StarterKit.configure({ undoRedo: false })];
const vocabulary = editorVocabulary(kit);
const tick = () => new Promise((resolve) => setTimeout(resolve, 0));
const mounted: { destroy(): void }[] = [];
afterEach(() => {
  for (const m of mounted.splice(0)) m.destroy();
});

test("opening and rebuilding an editor on URL-like text writes nothing", async () => {
  const body = fakeBinding(vocabulary, (root) =>
    root.insert(0, [paragraph("First line.abc")]),
  );
  // What the editor itself changes in the document: local transactions.
  const local: string[] = [];
  body.doc.on("afterTransaction", (tr: Y.Transaction) => {
    if (tr.local && tr.changed.size > 0) local.push("edit");
  });
  const element = document.createElement("div");
  document.body.append(element);
  let editors = 0;
  mounted.push(
    ncEditor(body, {
      element,
      extensions: kit,
      onEditor: () => {
        editors += 1;
      },
    }),
  );
  // A node the editor lacks arrives, and the editor is rebuilt.
  const other = new Y.Doc();
  Y.applyUpdate(other, Y.encodeStateAsUpdate(body.doc));
  const callout = new Y.XmlElement("callout");
  callout.insert(0, [paragraph("Later.")]);
  other.getXmlFragment("default").insert(1, [callout]);
  Y.applyUpdate(
    body.doc,
    Y.encodeStateAsUpdate(other, Y.encodeStateVector(body.doc)),
    "remote",
  );
  await tick();
  await tick();
  expect(editors).toBe(2);
  expect(local).toEqual([]);
  expect(body.doc.getXmlFragment("default").toString()).not.toContain("link");
});

test("a person's edit still reaches the document", async () => {
  const body = fakeBinding(vocabulary, (root) =>
    root.insert(0, [paragraph("Hello")]),
  );
  const element = document.createElement("div");
  document.body.append(element);
  let editor: import("@tiptap/core").Editor | undefined;
  mounted.push(
    ncEditor(body, {
      element,
      extensions: kit,
      onEditor: (e) => {
        editor = e;
      },
    }),
  );
  await tick();
  editor?.commands.insertContentAt(1, "Oh, ");
  expect(body.doc.getXmlFragment("default").toString()).toContain("Oh, Hello");
});
