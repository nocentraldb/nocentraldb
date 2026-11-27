// The guard on what the binding writes (RFC 0013, "An editor that cannot
// show a node", measure 3; NOC-461, 4c): an update that deletes an
// element, a format or an attribute the editor does not know is not
// written; the binding reloads its Y.Doc from the core once and reports
// the field not fully shown; a second refusal makes it read-only. Names
// the editor declares, placeholders included, are known, so a deliberate
// deletion passes.
import { expect, test, vi } from "vitest";
import * as Y from "yjs";
import { bind, PAUSE_MS, type RichTextPort } from "../src/richText/binding.ts";

const CLIENT = 1001;

/** A core whose document holds a paragraph and a callout, a node Anna's editor does not know. */
const fakeCore = () => {
  const core = new Y.Doc();
  const root = core.getXmlFragment("default");
  const p = new Y.XmlElement("paragraph");
  const words = new Y.XmlText();
  words.insert(0, "Hello");
  words.insert(5, " world", { highlight: { color: "yellow" } });
  p.insert(0, [words]);
  p.setAttribute("dir", "rtl");
  const callout = new Y.XmlElement("callout");
  root.insert(0, [p, callout]);
  const writes: Uint8Array[] = [];
  const port: RichTextPort = {
    pull: async (sv) => Y.encodeStateAsUpdate(core, sv),
    write: async (u) => {
      writes.push(u);
      Y.applyUpdate(core, u);
    },
    close: async () => {},
  };
  return { core, writes, port };
};

const known = {
  nodes: ["paragraph"],
  marks: ["bold"],
  attributes: { paragraph: [] as string[] },
};

const open = (core: Y.Doc, port: RichTextPort, k = known) =>
  bind(
    { client: CLIENT, state: Y.encodeStateAsUpdate(core), writable: true },
    port,
    () => {},
    { known: () => k },
  );

const root = (doc: Y.Doc) => doc.getXmlFragment("default");
const paragraphText = (doc: Y.Doc) =>
  (root(doc).get(0) as Y.XmlElement).get(0) as Y.XmlText;

const pause = async () => {
  await vi.advanceTimersByTimeAsync(PAUSE_MS);
  for (let i = 0; i < 10; i++) await Promise.resolve();
};

vi.useFakeTimers();

test("an update that deletes an element the editor does not know is not written, and the binding reloads once", async () => {
  const { core, writes, port } = fakeCore();
  const body = open(core, port);
  const reloads: Y.Doc[] = [];
  body.onReload((doc) => reloads.push(doc));
  root(body.doc).delete(1, 1);
  await pause();
  expect(writes).toHaveLength(0);
  expect(body.status).toEqual({
    status: "notFullyShown",
    reason: expect.stringContaining("callout"),
  });
  expect(reloads).toHaveLength(1);
  expect(body.doc).toBe(reloads[0]);
  expect(root(body.doc).toString()).toBe(root(core).toString());
  expect(body.doc.clientID).toBe(CLIENT);
});

test("a format the editor does not know, removed, is not written", async () => {
  const { core, writes, port } = fakeCore();
  const body = open(core, port);
  paragraphText(body.doc).format(5, 6, { highlight: null });
  await pause();
  expect(writes).toHaveLength(0);
  expect(body.status.status).toBe("notFullyShown");
});

test("an attribute the editor does not know, removed, is not written", async () => {
  const { core, writes, port } = fakeCore();
  const body = open(core, port);
  (root(body.doc).get(0) as Y.XmlElement).removeAttribute("dir");
  await pause();
  expect(writes).toHaveLength(0);
  expect(body.status.status).toBe("notFullyShown");
});

test("a second refusal makes the field read-only, and nothing more is written", async () => {
  const { core, writes, port } = fakeCore();
  const body = open(core, port);
  root(body.doc).delete(1, 1);
  await pause();
  root(body.doc).delete(1, 1);
  await pause();
  expect(body.status.status).toBe("readOnly");
  paragraphText(body.doc).insert(0, "More ");
  await pause();
  expect(writes).toHaveLength(0);
});

test("a deliberate deletion of a placeholder the editor declares is written", async () => {
  const { core, writes, port } = fakeCore();
  const body = open(core, port, {
    ...known,
    nodes: ["paragraph", "callout"],
    marks: ["bold", "highlight"],
    attributes: { paragraph: ["dir"] },
  });
  root(body.doc).delete(1, 1);
  await pause();
  expect(writes).toHaveLength(1);
  expect(body.status).toEqual({ status: "shown" });
  expect(root(core).toString()).not.toContain("callout");
});

test("ordinary typing and deleting known text is written", async () => {
  const { core, writes, port } = fakeCore();
  const body = open(core, port);
  paragraphText(body.doc).delete(0, 2);
  paragraphText(body.doc).insert(0, "J");
  await pause();
  expect(writes).toHaveLength(1);
  expect(body.status).toEqual({ status: "shown" });
});

test("a write the core refuses is reloaded, so the editor shows only what the core holds", async () => {
  const { core, port } = fakeCore();
  const refusing: RichTextPort = {
    ...port,
    write: async () => {
      throw new Error("notAllowed");
    },
  };
  const body = open(core, refusing);
  const reloads: Y.Doc[] = [];
  body.onReload((doc) => reloads.push(doc));
  paragraphText(body.doc).insert(0, "Not held ");
  await pause();
  expect(reloads).toHaveLength(1);
  expect(root(body.doc).toString()).toBe(root(core).toString());
  expect(body.status).toEqual({ status: "readOnly" });
});

test("names the editor declares with known(), its placeholders, pass the guard on top of the vocabulary", async () => {
  const { core, writes, port } = fakeCore();
  const body = open(core, port);
  body.known({ nodes: ["callout"], marks: [], attributes: {} });
  root(body.doc).delete(1, 1);
  await pause();
  expect(writes).toHaveLength(1);
  expect(body.status).toEqual({ status: "shown" });
});
