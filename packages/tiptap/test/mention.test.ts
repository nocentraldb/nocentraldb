// NcMention (RFC 0013, "Mentions"; NOC-461): an inline node named
// `mention` with the target's `space` and `id`, and no name. The editor
// shows the name the app's `label` gives for the target, a neutral
// placeholder until it has one, and the placeholder again when `label`
// gives nothing, as after an erasure. Its options' shape waits for
// NOC-579; this is option (a).
import { Editor } from "@tiptap/core";
import Collaboration from "@tiptap/extension-collaboration";
import StarterKit from "@tiptap/starter-kit";
import { afterEach, expect, test } from "vitest";
import * as Y from "yjs";
import { NcMention, type NcMentionTarget } from "../src/mention.ts";

const SPACE = "space-1";
const ADA = "object-ada";
const editors: Editor[] = [];
afterEach(() => {
  for (const editor of editors.splice(0)) editor.destroy();
});
const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

/** An editor on a fresh Y.Doc whose mentions take their names from `label`. */
const editorWith = (
  label: (
    target: NcMentionTarget,
  ) => string | undefined | Promise<string | undefined>,
) => {
  const doc = new Y.Doc();
  const element = document.createElement("div");
  document.body.append(element);
  const editor = new Editor({
    element,
    extensions: [
      StarterKit.configure({ undoRedo: false }),
      NcMention.configure({ label }),
      Collaboration.configure({ document: doc }),
    ],
  });
  editors.push(editor);
  editor.commands.insertContent([
    { type: "text", text: "Ask " },
    { type: "mention", attrs: { space: SPACE, id: ADA } },
  ]);
  const shown = () =>
    element.querySelector('[data-type="mention"]')?.textContent ?? null;
  return { doc, editor, shown };
};

test("stores the target's space and id, and no name", () => {
  const { doc, editor } = editorWith(() => "Ada Lovelace");
  const stored = doc.getXmlFragment("default").toString();
  expect(stored).toContain(`<mention id="${ADA}" space="${SPACE}"></mention>`);
  expect(stored).not.toContain("Ada Lovelace");
  expect(editor.getHTML()).not.toContain("Ada Lovelace");
  expect(editor.getJSON().content?.[0]?.content?.[1]).toEqual({
    type: "mention",
    attrs: { space: SPACE, id: ADA },
  });
});

test("shows the name label gives for the target", () => {
  const asked: NcMentionTarget[] = [];
  const { shown } = editorWith((target) => {
    asked.push(target);
    return "Ada Lovelace";
  });
  expect(shown()).toBe("@Ada Lovelace");
  expect(asked).toContainEqual({ space: SPACE, id: ADA });
});

test("shows a placeholder until a label that is a promise resolves", async () => {
  let resolve: (name: string) => void = () => {};
  const { shown } = editorWith(() => new Promise<string>((r) => (resolve = r)));
  expect(shown()).toBe("@…");
  resolve("Ada Lovelace");
  await tick();
  expect(shown()).toBe("@Ada Lovelace");
});

test("shows the placeholder when label gives nothing, as after an erasure", async () => {
  const { shown } = editorWith(() => undefined);
  expect(shown()).toBe("@…");
  const empty = editorWith(() => "");
  expect(empty.shown()).toBe("@…");
  const failing = editorWith(() => Promise.reject(new Error("gone")));
  await tick();
  expect(failing.shown()).toBe("@…");
});

test("reads a mention back from the HTML it renders", () => {
  const { editor } = editorWith(() => "Ada Lovelace");
  const html = editor.getHTML();
  editor.commands.setContent(html);
  expect(editor.getJSON().content?.[0]?.content?.[1]).toEqual({
    type: "mention",
    attrs: { space: SPACE, id: ADA },
  });
});
