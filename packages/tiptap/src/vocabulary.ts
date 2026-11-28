// The editor against the field's vocabulary (RFC 0013, "The vocabulary, at
// start"; Carsten, 2026-11-27, NOC-553): the editor's nodes and marks,
// without ProseMirror's own `doc` and `text`, must be the vocabulary's,
// or the editor is refused with `vocabularyMismatch` (NOC-538, Q3).

import { nc, type Vocabulary } from "@nocentraldb/sdk";
import { type AnyExtension, getSchema } from "@tiptap/core";

/** ProseMirror's own nodes, which a vocabulary never lists (NOC-558). */
const OWN = new Set(["doc", "text"]);

/** The nodes and marks of the editor `extensions` build, without `doc` and `text`, sorted. */
export const editorVocabulary = (extensions: AnyExtension[]): Vocabulary => {
  const schema = getSchema(extensions);
  return {
    nodes: Object.keys(schema.nodes)
      .filter((name) => !OWN.has(name))
      .sort(),
    marks: Object.keys(schema.marks).sort(),
  };
};

/** Throws `vocabularyMismatch` when the editor `extensions` build differs from `vocabulary`, naming what each side lacks. */
export const checkVocabulary = (
  extensions: AnyExtension[],
  vocabulary: Vocabulary,
): void => {
  const editor = editorVocabulary(extensions);
  const missing = (from: readonly string[], to: readonly string[]) =>
    from.filter((name) => !to.includes(name));
  const editorOnly = [
    ...missing(editor.nodes, vocabulary.nodes),
    ...missing(editor.marks, vocabulary.marks),
  ];
  const vocabularyOnly = [
    ...missing(vocabulary.nodes, editor.nodes),
    ...missing(vocabulary.marks, editor.marks),
  ];
  if (editorOnly.length === 0 && vocabularyOnly.length === 0) return;
  throw new nc.Error(
    "vocabularyMismatch",
    `the editor differs from the field's vocabulary: the editor has ${editorOnly.join(", ") || "nothing more"}, the vocabulary has ${vocabularyOnly.join(", ") || "nothing more"}; add the names to the schema or to the editor`,
  );
};
