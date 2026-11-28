// Measure 1 of RFC 0013, "An editor that cannot show a node": a schema
// widened from the document. Each node, mark and attribute the editor does
// not know becomes a placeholder type: a node of that name, inline or block
// as it sits, with its attributes declared without a default of their own;
// a mark with its attributes; an extra attribute on a known node. Each has
// parse rules, so copy and paste keep it (NOC-455).
import { getSchema } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import { expect, test } from "vitest";
import * as Y from "yjs";
import { widen } from "../src/widen.ts";

const kit = [StarterKit.configure({ undoRedo: false })];

/** A newer app's document: a callout, a chip in a paragraph, a highlight, and `dir` on a paragraph. */
const newer = () => {
  const doc = new Y.Doc();
  const root = doc.getXmlFragment("default");
  const p = new Y.XmlElement("paragraph");
  p.setAttribute("dir", "rtl");
  const before = new Y.XmlText();
  before.insert(0, "Hi ");
  before.insert(3, "there", { highlight: { color: "yellow" } });
  const chip = new Y.XmlElement("chip");
  chip.setAttribute("person", "p1");
  p.insert(0, [before, chip]);
  const callout = new Y.XmlElement("callout");
  callout.setAttribute("tone", "warn");
  const inner = new Y.XmlElement("paragraph");
  inner.insert(0, [new Y.XmlText("Careful.")]);
  callout.insert(0, [inner]);
  root.insert(0, [p, callout]);
  return doc;
};

const widened = () => {
  const extensions = [...kit, ...widen(newer(), kit)];
  return { schema: getSchema(extensions), extensions };
};

test("an unknown block node becomes a block placeholder with its attributes, without defaults", () => {
  const callout = widened().schema.nodes.callout;
  expect(callout?.isBlock).toBe(true);
  expect(Object.keys(callout?.spec.attrs ?? {})).toContain("tone");
  expect(callout?.spec.attrs?.tone?.default).toBeNull();
  expect(callout?.spec.parseDOM?.length).toBeGreaterThan(0);
});

test("an unknown node inside text becomes an inline placeholder", () => {
  const chip = widened().schema.nodes.chip;
  expect(chip?.isInline).toBe(true);
  expect(Object.keys(chip?.spec.attrs ?? {})).toContain("person");
  expect(chip?.spec.parseDOM?.length).toBeGreaterThan(0);
});

test("an unknown mark becomes a placeholder mark with its attributes", () => {
  const highlight = widened().schema.marks.highlight;
  expect(highlight).toBeDefined();
  expect(Object.keys(highlight?.spec.attrs ?? {})).toContain("color");
  expect(highlight?.spec.parseDOM?.length).toBeGreaterThan(0);
});

test("an attribute a known node does not declare is declared on it", () => {
  const paragraph = widened().schema.nodes.paragraph;
  expect(Object.keys(paragraph?.spec.attrs ?? {})).toContain("dir");
});

test("a document the editor knows widens nothing", () => {
  const doc = new Y.Doc();
  const p = new Y.XmlElement("paragraph");
  p.insert(0, [new Y.XmlText("Plain")]);
  doc.getXmlFragment("default").insert(0, [p]);
  expect(widen(doc, kit)).toEqual([]);
});
