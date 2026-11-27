// `space.richText(collection, id, field)` (RFC 0013, "An editor: the
// binding"; NOC-461, 4c): it opens a binding through the bridge's
// `richText.open`, with the field's vocabulary from the lock (NOC-553), and
// pulls after each fold of the object; a core without rich text answers
// `notSupported` (Carsten, 2026-11-27, NOC-538 Q5). Against a fake core.
import { expect, test } from "vitest";
import * as Y from "yjs";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { nc } from "../src/index.ts";

const model = nc.model({
  notes: nc.collection({ title: nc.string(), body: nc.richText() }),
});
const vocabulary = { nodes: ["paragraph"], marks: ["bold"] };
const lock = {
  model: { id: "00112233445566778899aabbccddeeff" },
  collections: {
    notes: {
      fields: {
        body: { type: "ncdb/richText", rule: 1, since: 1, vocabulary },
      },
    },
  },
};
const SPACE = "0123456789abcdef0123456789abcdef";
const NOTE = "fedcba9876543210fedcba9876543210";

const fake = (richText: boolean) => {
  let listener: ((event: Uint8Array) => void) | undefined;
  const calls: { call: string; args: Record<string, unknown> }[] = [];
  const state = new Y.Doc();
  const core: Core = {
    call: async (bytes) => {
      const { id, call, args } = decode(bytes) as {
        id: number;
        call: string;
        args: Record<string, unknown>;
      };
      calls.push({ call, args });
      switch (call) {
        case "open":
          return encode({ id, ok: { sync: null, richText } });
        case "spaces.get":
          return encode({ id, ok: args.space });
        case "richText.open":
          return encode({
            id,
            ok: {
              lease: 4,
              client: 77,
              clock: 0,
              writable: true,
              state: Y.encodeStateAsUpdate(state),
            },
          });
        case "richText.pull":
          return encode({
            id,
            ok: Y.encodeStateAsUpdate(state, args.stateVector as Uint8Array),
          });
        default:
          return encode({ id, ok: null });
      }
    },
    terminate: () => {},
    listen: (given) => {
      listener = given;
    },
  };
  const send = (message: Record<string, unknown>) =>
    listener?.(encode(message));
  return { core, calls, send, state };
};

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

test("a core without rich text answers notSupported", async () => {
  const { core } = fake(false);
  const db = await connect(core, { model, lock, inMemory: true } as never);
  const space = await db.spaces.get(SPACE);
  await expect(space.richText(model.notes, NOTE, "body")).rejects.toMatchObject(
    { code: "notSupported" },
  );
});

test("it opens a binding on the field, with the lock's vocabulary", async () => {
  const { core, calls } = fake(true);
  const db = await connect(core, { model, lock, inMemory: true } as never);
  const space = await db.spaces.get(SPACE);
  const body = await space.richText(model.notes, NOTE, "body");
  expect(calls.find((c) => c.call === "richText.open")?.args).toEqual({
    space: SPACE,
    collection: "notes",
    id: NOTE,
    field: "body",
  });
  expect(body.doc.clientID).toBe(77);
  expect(body.writable).toBe(true);
  expect(body.vocabulary).toEqual(vocabulary);
  expect(body.status).toEqual({ status: "shown" });
});

test("a fold of the object pulls what the binding lacks; a fold of another does not", async () => {
  const { core, calls, send, state } = fake(true);
  const db = await connect(core, { model, lock, inMemory: true } as never);
  const space = await db.spaces.get(SPACE);
  const body = await space.richText(model.notes, NOTE, "body");
  const fold = (ids: string[]) =>
    send({
      event: "folded",
      folds: [
        {
          space: SPACE,
          collections: ["notes"],
          changes: [{ collection: "notes", ids, origin: "remote" }],
        },
      ],
    });
  fold(["00000000000000000000000000000000"]);
  await settle();
  expect(calls.filter((c) => c.call === "richText.pull")).toHaveLength(0);
  const p = new Y.XmlElement("paragraph");
  state.getXmlFragment("default").insert(0, [p]);
  fold([NOTE]);
  await settle();
  expect(calls.filter((c) => c.call === "richText.pull")).toHaveLength(1);
  expect(body.doc.getXmlFragment("default").toString()).toBe(
    "<paragraph></paragraph>",
  );
});

test("close gives the lease back and stops pulling", async () => {
  const { core, calls, send } = fake(true);
  const db = await connect(core, { model, lock, inMemory: true } as never);
  const space = await db.spaces.get(SPACE);
  const body = await space.richText(model.notes, NOTE, "body");
  await body.close();
  expect(calls.at(-1)).toEqual({ call: "richText.close", args: { lease: 4 } });
  send({
    event: "folded",
    folds: [
      {
        space: SPACE,
        collections: ["notes"],
        changes: [{ collection: "notes", ids: [NOTE], origin: "remote" }],
      },
    ],
  });
  await settle();
  expect(calls.filter((c) => c.call === "richText.pull")).toHaveLength(0);
});

test("hiding the page writes at once, and the guard checks deletions against the vocabulary", async () => {
  const listeners = new Map<string, () => void>();
  const page = {
    visibilityState: "visible",
    addEventListener: (type: string, listener: () => void) =>
      listeners.set(type, listener),
    removeEventListener: (type: string) => listeners.delete(type),
  };
  (globalThis as { document?: unknown }).document = page;
  try {
    const { core, calls, state } = fake(true);
    const callout = new Y.XmlElement("callout");
    const p = new Y.XmlElement("paragraph");
    state.getXmlFragment("default").insert(0, [p, callout]);
    const db = await connect(core, { model, lock, inMemory: true } as never);
    const space = await db.spaces.get(SPACE);
    const body = await space.richText(model.notes, NOTE, "body");
    const root = body.doc.getXmlFragment("default");
    (root.get(0) as Y.XmlElement).insert(0, [new Y.XmlText("Hi")]);
    page.visibilityState = "hidden";
    listeners.get("visibilitychange")?.();
    await settle();
    expect(calls.filter((c) => c.call === "richText.write")).toHaveLength(1);
    root.delete(1, 1);
    listeners.get("visibilitychange")?.();
    await settle();
    expect(calls.filter((c) => c.call === "richText.write")).toHaveLength(1);
    expect(body.status.status).toBe("notFullyShown");
    await body.close();
    expect(listeners.has("visibilitychange")).toBe(false);
  } finally {
    delete (globalThis as { document?: unknown }).document;
  }
});
