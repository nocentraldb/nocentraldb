// The editor against the field's vocabulary (RFC 0013, "The vocabulary, at
// start"; Carsten, 2026-11-27, NOC-553): leaving out `doc` and `text`, an
// editor whose nodes or marks differ from the vocabulary, in either
// direction, is refused with `vocabularyMismatch`, which names the names on
// each side (NOC-538, Q3).
import StarterKit from "@tiptap/starter-kit";
import { expect, test } from "vitest";
import { checkVocabulary, editorVocabulary } from "../src/vocabulary.ts";

const kit = [StarterKit.configure({ undoRedo: false })];

test("the editor's vocabulary leaves out doc and text, sorted", () => {
  const { nodes, marks } = editorVocabulary(kit);
  expect(nodes).not.toContain("doc");
  expect(nodes).not.toContain("text");
  expect(nodes).toContain("paragraph");
  expect(marks).toContain("bold");
  expect([...nodes].sort()).toEqual(nodes);
});

test("an editor with the vocabulary's names passes", () => {
  expect(() => checkVocabulary(kit, editorVocabulary(kit))).not.toThrow();
});

test("an editor with a name the vocabulary lacks is refused, naming it", () => {
  const vocabulary = editorVocabulary(kit);
  const narrower = {
    nodes: vocabulary.nodes.filter((n) => n !== "blockquote"),
    marks: vocabulary.marks,
  };
  expect(() => checkVocabulary(kit, narrower)).toThrowError(
    expect.objectContaining({
      code: "vocabularyMismatch",
      message: expect.stringContaining("blockquote"),
    }),
  );
});

test("a vocabulary with a name the editor lacks is refused, naming it", () => {
  const vocabulary = editorVocabulary(kit);
  const wider = {
    nodes: vocabulary.nodes,
    marks: [...vocabulary.marks, "highlight"],
  };
  expect(() => checkVocabulary(kit, wider)).toThrowError(
    expect.objectContaining({
      code: "vocabularyMismatch",
      message: expect.stringContaining("highlight"),
    }),
  );
});
