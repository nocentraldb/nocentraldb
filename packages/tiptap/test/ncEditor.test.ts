// `ncEditor(body, { element, extensions, onEditor })` (RFC 0013, "An editor
// that cannot show a node"; NOC-538 Q4, b; NOC-555): it builds the TipTap
// editor with the content check, the binding's Y.Doc and its one
// UndoManager, widens the schema from the document, tells the binding the
// placeholders, rebuilds when a new kind of node arrives or the binding
// reloads, hands each editor to `onEditor`, and makes the editor read-only
// when the binding is.
import type { Editor } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import { ySyncPluginKey } from "@tiptap/y-tiptap";
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

const mount = (body = fakeBinding(vocabulary)) => {
  const editors: Editor[] = [];
  const element = document.createElement("div");
  document.body.append(element);
  mounted.push(
    ncEditor(body, {
      element,
      extensions: kit,
      onEditor: (editor) => editors.push(editor),
    }),
  );
  return { body, editors, element };
};

test("it hands the editor to onEditor, with the content check, the binding's document and its UndoManager", () => {
  const { body, editors } = mount(
    fakeBinding(vocabulary, (root) => root.insert(0, [paragraph("Hello")])),
  );
  expect(editors).toHaveLength(1);
  const editor = editors[0] as Editor;
  expect(editor.options.enableContentCheck).toBe(true);
  expect(editor.getText()).toBe("Hello");
  expect(body.undoManager.trackedOrigins.has(ySyncPluginKey)).toBe(true);
});

test("an editor that differs from the vocabulary is refused at start", () => {
  const body = fakeBinding({ nodes: ["paragraph"], marks: [] });
  expect(() => mount(body)).toThrowError(
    expect.objectContaining({ code: "vocabularyMismatch" }),
  );
});

test("a node the editor lacks is kept as a placeholder, and the binding learns its name", () => {
  const { body, editors } = mount(
    fakeBinding(vocabulary, (root) => {
      const callout = new Y.XmlElement("callout");
      callout.setAttribute("tone", "warn");
      callout.insert(0, [paragraph("Careful.")]);
      root.insert(0, [paragraph("Before"), callout]);
    }),
  );
  expect(body.doc.getXmlFragment("default").toString()).toContain(
    '<callout tone="warn">',
  );
  expect((editors[0] as Editor).getText()).toContain("Careful.");
  expect(body.declared?.nodes).toEqual(["callout"]);
});

test("a new kind of node arriving rebuilds the editor, and onEditor gets the new one", async () => {
  const { body, editors } = mount(
    fakeBinding(vocabulary, (root) => root.insert(0, [paragraph("Hi")])),
  );
  const callout = new Y.XmlElement("callout");
  callout.insert(0, [paragraph("Later.")]);
  const other = new Y.Doc();
  Y.applyUpdate(other, Y.encodeStateAsUpdate(body.doc));
  other.getXmlFragment("default").insert(1, [callout]);
  Y.applyUpdate(
    body.doc,
    Y.encodeStateAsUpdate(other, Y.encodeStateVector(body.doc)),
  );
  await tick();
  await tick();
  expect(editors).toHaveLength(2);
  expect((editors[0] as Editor).isDestroyed).toBe(true);
  expect((editors[1] as Editor).getText()).toContain("Later.");
  expect(body.doc.getXmlFragment("default").toString()).toContain("<callout>");
  expect(body.declared?.nodes).toEqual(["callout"]);
});

test("a reload of the binding rebuilds the editor on the new document", () => {
  const { body, editors } = mount();
  const fresh = new Y.Doc();
  fresh.getXmlFragment("default").insert(0, [paragraph("Reloaded")]);
  body.reload(fresh);
  expect(editors).toHaveLength(2);
  expect((editors[1] as Editor).getText()).toBe("Reloaded");
});

test("the editor turns read-only when the binding does", () => {
  const { body, editors } = mount();
  body.setStatus({ status: "readOnly" });
  expect((editors.at(-1) as Editor).isEditable).toBe(false);
});

test("a composition in the editor reaches the binding", () => {
  const { body, element } = mount();
  element.dispatchEvent(new Event("compositionstart", { bubbles: true }));
  element.dispatchEvent(new Event("compositionend", { bubbles: true }));
  expect(body.compositions).toEqual([true, false]);
});
