// A document that holds a node or a mark the field's vocabulary does not
// declare, written by a newer version of the app, is reported as not
// fully shown, with the reason that the content uses names this version of
// the app does not declare (RFC 0013, "The vocabulary, at start";
// Carsten, 2026-11-27, NOC-553).
import { expect, test } from "vitest";
import * as Y from "yjs";
import { bind, type RichTextPort } from "../src/richText/binding.ts";

const vocabulary = { nodes: ["paragraph"], marks: ["bold"] };

const coreWith = (build: (root: Y.XmlFragment) => void) => {
  const core = new Y.Doc();
  build(core.getXmlFragment("default"));
  const port: RichTextPort = {
    pull: async (sv) => Y.encodeStateAsUpdate(core, sv),
    write: async (u) => Y.applyUpdate(core, u),
    close: async () => {},
  };
  return { core, port };
};

const paragraph = (words: string, marks?: Record<string, unknown>) => {
  const p = new Y.XmlElement("paragraph");
  const t = new Y.XmlText();
  t.insert(0, words, marks);
  p.insert(0, [t]);
  return p;
};

const open = (core: Y.Doc, port: RichTextPort, writable = true) =>
  bind(
    { client: 7, state: Y.encodeStateAsUpdate(core), writable },
    port,
    () => {},
    {
      vocabulary,
    },
  );

test("a document in the vocabulary is shown", () => {
  const { core, port } = coreWith((r) =>
    r.insert(0, [paragraph("Hi", { bold: true })]),
  );
  expect(open(core, port).status).toEqual({ status: "shown" });
});

test("a node or a mark outside the vocabulary is reported at open, with the names", () => {
  const { core, port } = coreWith((r) =>
    r.insert(0, [
      paragraph("Hi", { highlight: { color: "red" } }),
      new Y.XmlElement("callout"),
    ]),
  );
  const status = open(core, port).status;
  expect(status.status).toBe("notFullyShown");
  expect(status.status === "notFullyShown" && status.reason).toMatch(
    /does not declare: callout, highlight$/,
  );
});

test("a node outside the vocabulary that arrives later is reported after the pull", async () => {
  const { core, port } = coreWith((r) => r.insert(0, [paragraph("Hi")]));
  const body = open(core, port);
  core.getXmlFragment("default").insert(1, [new Y.XmlElement("callout")]);
  await body.folded();
  expect(body.status.status).toBe("notFullyShown");
});

test("a reader's binding stays readOnly", () => {
  const { core, port } = coreWith((r) =>
    r.insert(0, [new Y.XmlElement("callout")]),
  );
  expect(open(core, port, false).status).toEqual({ status: "readOnly" });
});
