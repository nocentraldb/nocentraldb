// db.durability.waitingForTab (NOC-355): while this tab waits for the
// database's storage, which another tab of the app still holds, as a
// frozen owner tab in Safari does (NOC-352), the app learns it, so that it
// can tell the user why nothing is saved yet. The worker reports the wait
// as the event `waitingForTab` (worker.ts); connect.ts keeps it in the
// durability report. Against a fake core.
import { afterEach, expect, test, vi } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { nc } from "../src/index.ts";

const model = nc.model({
  tasks: nc.collection({ title: nc.string() }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };

/** The durability as the core sends it: the core knows nothing of tabs. */
const CORE = {
  storage: "best-effort",
  unconfirmed: 0,
  oldestUnconfirmedAt: null,
  atRisk: false,
};

const fake = () => {
  let listener: ((event: Uint8Array) => void) | undefined;
  const core: Core = {
    call: async (bytes) => {
      const { id, call } = decode(bytes) as { id: number; call: string };
      return call === "open"
        ? encode({
            id,
            ok: {
              sync: { connection: "none" },
              durability: CORE,
              view: { rebuild: null },
            },
          })
        : encode({ id, ok: null });
    },
    terminate: () => {},
    listen: (given) => {
      listener = given;
    },
  };
  const send = (message: Record<string, unknown>) =>
    listener?.(encode(message));
  return { core, send };
};

afterEach(() => {
  vi.unstubAllGlobals();
});

const opened = async () => {
  vi.stubGlobal("navigator", {});
  const { core, send } = fake();
  const db = await connect(core, { model, lock });
  return { db, send };
};

test("a database that waits for no tab reports waitingForTab: false", async () => {
  const { db } = await opened();
  expect(db.durability.current()).toEqual({ ...CORE, waitingForTab: false });
});

test("while the worker waits for another tab's storage, waitingForTab is true, and false again once it has it", async () => {
  const { db, send } = await opened();
  const seen: boolean[] = [];
  db.durability.subscribe((durability) => seen.push(durability.waitingForTab));
  send({ event: "waitingForTab", waiting: true });
  expect(db.durability.current().waitingForTab).toBe(true);
  send({ event: "waitingForTab", waiting: false });
  expect(db.durability.current().waitingForTab).toBe(false);
  expect(seen).toEqual([false, true, false]);
});

test("a durability the core sends while this tab waits keeps waitingForTab", async () => {
  const { db, send } = await opened();
  send({ event: "waitingForTab", waiting: true });
  send({ event: "durability", durability: { ...CORE, unconfirmed: 1 } });
  expect(db.durability.current()).toMatchObject({
    unconfirmed: 1,
    waitingForTab: true,
  });
});
