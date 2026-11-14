// db.durability and db.view (NOC-315, slice 3; RFC 0006, "What the SDK
// reports"; RFC 0009, "Sync and status"; the shapes the owner decided on
// NOC-317, Carsten, 2026-11-14). `open` answers both, the core's
// `durability` and `view` events keep them current, and the page asks the
// browser for persistent storage (crates/nocentraldb/src/bridge/mod.rs).
// Against a fake core.
import { afterEach, expect, expectTypeOf, test, vi } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { type Durability, nc } from "../src/index.ts";

const model = nc.model({
  tasks: nc.collection({ title: nc.string() }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };

const BEST = {
  storage: "best-effort",
  unconfirmed: 0,
  oldestUnconfirmedAt: null,
  atRisk: false,
};
const PERSISTENT = { ...BEST, storage: "persistent" };

const fake = (refreshed: Record<string, unknown> = PERSISTENT) => {
  let listener: ((event: Uint8Array) => void) | undefined;
  const calls: string[] = [];
  const core: Core = {
    call: async (bytes) => {
      const { id, call } = decode(bytes) as { id: number; call: string };
      calls.push(call);
      switch (call) {
        case "open":
          return encode({
            id,
            ok: {
              sync: { connection: "none" },
              durability: BEST,
              view: { rebuild: null },
            },
          });
        case "durability":
          return encode({ id, ok: refreshed });
        case "view":
          return encode({ id, ok: { rebuild: { folded: 3, total: 10 } } });
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
  return { core, calls, send };
};

/** A browser whose `navigator.storage.persist()` answers `answer`. */
const browser = (answer: boolean) => {
  const persist = vi.fn(async () => answer);
  vi.stubGlobal("navigator", { storage: { persist } });
  return persist;
};

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

afterEach(() => {
  vi.unstubAllGlobals();
});

test("db.durability.current() is the durability open answered", async () => {
  browser(false);
  const { core } = fake();
  const db = await connect(core, { model, lock });
  expect(db.durability.current()).toEqual(BEST);
});

test("subscribe gives the durability at once and each new one the core sends", async () => {
  browser(false);
  const { core, send } = fake();
  const db = await connect(core, { model, lock });
  const seen: Durability[] = [];
  const stop = db.durability.subscribe((durability) => seen.push(durability));
  const risky = {
    ...BEST,
    unconfirmed: 2,
    oldestUnconfirmedAt: 1_763_000_000_000,
    atRisk: true,
  };
  send({ event: "durability", durability: risky });
  stop();
  send({ event: "durability", durability: BEST });
  expect(seen).toEqual([BEST, risky]);
  expect(db.durability.current()).toEqual(BEST);
});

test("nc.open asks the browser for persistent storage without waiting, and reports its answer", async () => {
  const persist = browser(true);
  const { core, calls } = fake();
  const db = await connect(core, { model, lock });
  expect(persist).toHaveBeenCalledTimes(1);
  await settle();
  expect(calls).toContain("durability");
  expect(db.durability.current().storage).toBe("persistent");
});

test("with persistentStorage: on-request, nc.open does not ask; requestPersistent() does and answers the browser's answer", async () => {
  const persist = browser(true);
  const { core } = fake();
  const db = await connect(core, {
    model,
    lock,
    persistentStorage: "on-request",
  });
  await settle();
  expect(persist).not.toHaveBeenCalled();
  expect(db.durability.current().storage).toBe("best-effort");
  expect(await db.durability.requestPersistent()).toBe(true);
  expect(persist).toHaveBeenCalledTimes(1);
  expect(db.durability.current().storage).toBe("persistent");
});

test("requestPersistent() answers false where the browser has no persist()", async () => {
  vi.stubGlobal("navigator", {});
  const { core } = fake(BEST);
  const db = await connect(core, {
    model,
    lock,
    persistentStorage: "on-request",
  });
  expect(await db.durability.requestPersistent()).toBe(false);
});

test("db.view gives the view's report at once and each new one the core sends", async () => {
  browser(false);
  const { core, send } = fake();
  const db = await connect(core, { model, lock });
  expect(db.view.current()).toEqual({ rebuild: null });
  const seen: unknown[] = [];
  db.view.subscribe((view) => seen.push(view));
  send({ event: "view", view: { rebuild: { folded: 1, total: 4 } } });
  expect(seen).toEqual([
    { rebuild: null },
    { rebuild: { folded: 1, total: 4 } },
  ]);
});

test("after another tab took the database over, durability and view are read again", async () => {
  browser(false);
  const { core, calls, send } = fake();
  const db = await connect(core, {
    model,
    lock,
    persistentStorage: "on-request",
  });
  send({ event: "reopened", sync: { connection: "none" } });
  await settle();
  expect(calls.filter((call) => call === "durability")).toHaveLength(1);
  expect(calls.filter((call) => call === "view")).toHaveLength(1);
  expect(db.view.current()).toEqual({ rebuild: { folded: 3, total: 10 } });
});

test("their types are what the owner decided", async () => {
  browser(false);
  const { core } = fake();
  const db = await connect(core, { model, lock });
  expectTypeOf<Durability>().toEqualTypeOf<{
    readonly storage: "persistent" | "best-effort" | "temporary";
    readonly unconfirmed: number;
    readonly oldestUnconfirmedAt: number | null;
    readonly atRisk: boolean;
  }>();
  expectTypeOf(db.durability.current).returns.toEqualTypeOf<Durability>();
  expectTypeOf(db.durability.requestPersistent).returns.toEqualTypeOf<
    Promise<boolean>
  >();
  expectTypeOf(db.view.current).returns.toEqualTypeOf<{
    readonly rebuild: {
      readonly folded: number;
      readonly total: number;
    } | null;
  }>();
});

test("when the page becomes visible again, the durability is read again", async () => {
  browser(false);
  const handlers: Record<string, () => void> = {};
  vi.stubGlobal("document", {
    visibilityState: "visible",
    addEventListener: (kind: string, handler: () => void) => {
      handlers[kind] = handler;
    },
    removeEventListener: () => {},
  });
  const { core, calls } = fake();
  const db = await connect(core, {
    model,
    lock,
    persistentStorage: "on-request",
  });
  handlers.visibilitychange?.();
  await settle();
  expect(calls.filter((call) => call === "durability")).toHaveLength(1);
  expect(db.durability.current().storage).toBe("persistent");
});
