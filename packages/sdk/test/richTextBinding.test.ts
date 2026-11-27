// The rich-text editor binding's Y.Doc (RFC 0013, "An editor: the
// binding"; NOC-461, 4b): it writes the editor's edits as one change at
// each pause in typing, holds a write while an IME composition is open
// (Carsten, 2026-11-22, NOC-467), applies what the core folded under an
// origin of its own, which it never writes back, and keeps one
// UndoManager that takes back only this binding's own edits. A fake core
// stands in for the bridge: it applies each write to a Yjs document of its
// own and answers a pull from it.
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import * as Y from "yjs";
import {
  bind,
  MAX_HOLD_MS,
  PAUSE_MS,
  type RichTextPort,
} from "../src/richText/binding.ts";

const CLIENT = 1001;

/** The core's document, the writes it took, and a port onto it. */
const fakeCore = (refuse = false) => {
  const core = new Y.Doc();
  const writes: Uint8Array[] = [];
  const port: RichTextPort = {
    pull: async (stateVector) => Y.encodeStateAsUpdate(core, stateVector),
    write: async (update) => {
      if (refuse) throw new Error("refused");
      writes.push(update);
      Y.applyUpdate(core, update);
    },
    close: async () => {},
  };
  return { core, writes, port };
};

const opened = (core: Y.Doc, writable = true) => ({
  client: CLIENT,
  state: Y.encodeStateAsUpdate(core),
  writable,
});

const text = (doc: Y.Doc) => doc.getXmlFragment("default").toString();

/** Types `words` into a paragraph at the end, as an editor would. */
const type = (doc: Y.Doc, words: string) => {
  const root = doc.getXmlFragment("default");
  doc.transact(() => root.insert(root.length, [new Y.XmlText(words)]));
};

/** Lets the binding's promises settle. */
const settle = async () => {
  for (let i = 0; i < 10; i++) await Promise.resolve();
};

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
});

describe("the doc", () => {
  test("is made under the leased client id and loaded from the core's state", () => {
    const { core, port } = fakeCore();
    type(core, "Before");
    const body = bind(opened(core), port);
    expect(body.doc.clientID).toBe(CLIENT);
    expect(text(body.doc)).toBe("Before");
  });
});

describe("the write at each pause", () => {
  test("waits for 500 ms without an edit, then writes one change", async () => {
    const { core, writes, port } = fakeCore();
    const body = bind(opened(core), port);
    type(body.doc, "H");
    type(body.doc, "i");
    await vi.advanceTimersByTimeAsync(PAUSE_MS - 1);
    expect(writes).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(1);
    await settle();
    expect(writes).toHaveLength(1);
    expect(text(core)).toBe(text(body.doc));
  });

  test("writes after at most 2 s of typing without a pause", async () => {
    const { core, writes, port } = fakeCore();
    const body = bind(opened(core), port);
    for (let t = 0; t < MAX_HOLD_MS; t += 100) {
      type(body.doc, "x");
      await vi.advanceTimersByTimeAsync(100);
    }
    await settle();
    expect(writes).toHaveLength(1);
  });

  test("writes at once when the page is hidden", async () => {
    const { core, writes, port } = fakeCore();
    const body = bind(opened(core), port);
    type(body.doc, "Bye");
    await body.hidden();
    expect(writes).toHaveLength(1);
    expect(text(core)).toBe(text(body.doc));
  });

  test("writes what is left when the binding closes, then closes the port", async () => {
    const { core, writes, port } = fakeCore();
    const close = vi.spyOn(port, "close");
    const body = bind(opened(core), port);
    type(body.doc, "Last words");
    await body.close();
    expect(writes).toHaveLength(1);
    expect(close).toHaveBeenCalledOnce();
  });

  test("writes nothing for a reader's binding", async () => {
    const { core, writes, port } = fakeCore();
    const body = bind(opened(core, false), port);
    expect(body.writable).toBe(false);
    type(body.doc, "Not mine to write");
    await vi.advanceTimersByTimeAsync(MAX_HOLD_MS * 2);
    await body.close();
    expect(writes).toHaveLength(0);
  });
});

describe("what other members write", () => {
  test("is pulled from the core and never written back", async () => {
    const { core, writes, port } = fakeCore();
    const body = bind(opened(core), port);
    const other = new Y.Doc();
    Y.applyUpdate(other, Y.encodeStateAsUpdate(core));
    type(other, "From Bob");
    Y.applyUpdate(core, Y.encodeStateAsUpdate(other));
    await body.folded();
    expect(text(body.doc)).toBe(text(core));
    await vi.advanceTimersByTimeAsync(MAX_HOLD_MS * 2);
    await settle();
    expect(writes).toHaveLength(0);
  });

  test("a write holds only this binding's own client id", async () => {
    const { core, writes, port } = fakeCore();
    const body = bind(opened(core), port);
    const other = new Y.Doc();
    type(other, "Theirs");
    Y.applyUpdate(core, Y.encodeStateAsUpdate(other));
    await body.folded();
    type(body.doc, "Mine");
    await vi.advanceTimersByTimeAsync(PAUSE_MS);
    await settle();
    expect(writes).toHaveLength(1);
    const clients = [
      ...Y.decodeStateVector(
        Y.encodeStateVectorFromUpdate(Y.mergeUpdates(writes)),
      ).keys(),
    ];
    expect(clients).toEqual([CLIENT]);
  });
});

describe("an IME composition", () => {
  test("holds the write until compositionend", async () => {
    const { core, writes, port } = fakeCore();
    const body = bind(opened(core), port);
    body.composing(true);
    type(body.doc, "か");
    await vi.advanceTimersByTimeAsync(PAUSE_MS * 2);
    await settle();
    expect(writes).toHaveLength(0);
    body.composing(false);
    await settle();
    expect(writes).toHaveLength(1);
  });

  test("holds it for at most 2 s", async () => {
    const { core, writes, port } = fakeCore();
    const body = bind(opened(core), port);
    body.composing(true);
    type(body.doc, "か");
    await vi.advanceTimersByTimeAsync(MAX_HOLD_MS - 1);
    expect(writes).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(1);
    await settle();
    expect(writes).toHaveLength(1);
  });
});

describe("the UndoManager", () => {
  test("takes back this binding's edit, and the undo is written too", async () => {
    const { core, writes, port } = fakeCore();
    const body = bind(opened(core), port);
    type(body.doc, "Oops");
    await vi.advanceTimersByTimeAsync(PAUSE_MS);
    await settle();
    body.undoManager.undo();
    expect(text(body.doc)).toBe("");
    await vi.advanceTimersByTimeAsync(PAUSE_MS);
    await settle();
    expect(writes).toHaveLength(2);
    expect(text(core)).toBe("");
  });

  test("never takes back what another member wrote", async () => {
    const { core, port } = fakeCore();
    const body = bind(opened(core), port);
    const other = new Y.Doc();
    type(other, "Bob's");
    Y.applyUpdate(core, Y.encodeStateAsUpdate(other));
    await body.folded();
    expect(body.undoManager.canUndo()).toBe(false);
    body.undoManager.undo();
    expect(text(body.doc)).toBe(text(core));
  });
});

describe("the status (NOC-538, Q2)", () => {
  test("is shown while every write goes through", async () => {
    const { core, port } = fakeCore();
    const body = bind(opened(core), port);
    type(body.doc, "Fine");
    await body.hidden();
    expect(body.status).toEqual({ status: "shown" });
  });

  test("turns read-only when the core refuses a write, and tells onStatus once", async () => {
    const { core, writes, port } = fakeCore(true);
    const body = bind(opened(core), port);
    const heard: unknown[] = [];
    body.onStatus((status) => heard.push(status));
    type(body.doc, "Refused");
    await body.hidden();
    expect(body.status).toEqual({ status: "readOnly" });
    expect(heard).toEqual([{ status: "readOnly" }]);
    type(body.doc, "More");
    await body.hidden();
    expect(writes).toHaveLength(0);
    expect(heard).toHaveLength(1);
  });

  test("a listener that stops listening hears nothing more", async () => {
    const { core, port } = fakeCore(true);
    const body = bind(opened(core), port);
    const heard: unknown[] = [];
    const stop = body.onStatus((status) => heard.push(status));
    stop();
    type(body.doc, "Refused");
    await body.hidden();
    expect(heard).toEqual([]);
  });
});

test("a reader's binding is read-only from the start", () => {
  const { core, port } = fakeCore();
  const body = bind(opened(core, false), port);
  expect(body.status).toEqual({ status: "readOnly" });
});
