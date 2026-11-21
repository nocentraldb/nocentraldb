// One run of the main scenario: the newer app writes a document with parts
// the older app cannot interpret; the older app opens it in an editor, a person
// types in a paragraph the older app knows, then in the paragraph that carries
// an attribute it does not know; the edits sync back to the newer app.
import { prosemirrorJSONToYXmlFragment } from "y-prosemirror";
import {
  describeDeletes,
  newDocJSON,
  newMember,
  newSchema,
  oldMember,
  survivors,
  sync,
  xml,
} from "./setup.mjs";

export const writeNewDoc = () => {
  const doc = newMember();
  prosemirrorJSONToYXmlFragment(
    newSchema,
    newDocJSON,
    doc.getXmlFragment("default"),
  );
  return doc;
};

export const runScenario = (openEditor) => {
  const writer = writeNewDoc();
  const reader = oldMember();
  sync(writer, reader);
  const result = { beforeEditor: survivors(reader) };

  const deletes = [];
  const record = (step) => (tr) => {
    const d = describeDeletes(tr);
    if (d.length > 0) deletes.push({ step, origin: tr.local, deleted: d });
  };

  let listener = record("open");
  const onTx = (tr) => listener(tr);
  reader.on("afterTransaction", onTx);

  const ed = openEditor(reader);
  result.afterOpen = survivors(reader);
  result.shownAfterOpen = ed.shown();

  listener = record("type in a known paragraph");
  result.typeKnown = ed.type("Intro.", "Hi. ");
  result.afterTypeKnown = survivors(reader);
  result.typedTextInYjs = xml(reader).includes("Hi. Intro.");

  listener = record("type in the paragraph with an unknown attribute");
  result.typeAttr = ed.type("Centered.", "Now ");
  result.afterTypeAttr = survivors(reader);

  reader.off("afterTransaction", onTx);
  sync(reader, writer);
  result.writerAfterSync = survivors(writer);
  result.deletes = deletes;
  result.errors = ed.errors?.() ?? [];
  result.readerXml = xml(reader);
  ed.destroy();
  return result;
};
