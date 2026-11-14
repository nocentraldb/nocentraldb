// The space list as a report, and leaving a space (NOC-315, slice 1;
// RFC 0009, "Spaces"). `db.spaces.subscribe` asks the core's
// `spaces.follow` once and keeps the list current from the core's `spaces`
// events; `space.leave` is the core's `space.leave`
// (crates/nocentraldb/src/bridge/mod.rs). Against a fake core.
import { expect, expectTypeOf, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { type Notice, nc } from "../src/index.ts";

const model = nc.model({
  tasks: nc.collection({ title: nc.string() }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };
const SPACE = "0123456789abcdef0123456789abcdef";
const OTHER = "fedcba9876543210fedcba9876543210";

const ONE = [{ id: SPACE, role: "admin", model: "ours" }];
const LEFT = {
  code: "spaceLeft",
  text: "This member keeps its copy of everything it received before now. Until an admin rotates the space key, the others write under a key it still holds.",
};

const fake = (list: unknown[] = ONE) => {
  let listener: ((event: Uint8Array) => void) | undefined;
  const calls: { call: string; args: Record<string, unknown> }[] = [];
  const core: Core = {
    call: async (bytes) => {
      const { id, call, args } = decode(bytes) as {
        id: number;
        call: string;
        args: Record<string, unknown>;
      };
      calls.push({ call, args });
      switch (call) {
        case "spaces.list":
        case "spaces.follow":
          return encode({ id, ok: list });
        case "spaces.get":
          return encode({ id, ok: args.space });
        case "space.status":
          return encode({ id, ok: { role: "admin" } });
        case "space.leave":
          return encode({ id, ok: { notice: LEFT } });
        default:
          return encode({ id, ok: null });
      }
    },
    terminate: () => {},
    listen: (given) => {
      listener = given;
    },
  };
  const send = (message: Record<string, unknown>) => listener?.(encode(message));
  return { core, calls, send };
};

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

test("db.spaces.list gives each space's model", async () => {
  const { core } = fake([
    { id: SPACE, role: "admin", model: "ours" },
    { id: OTHER, role: null, model: "another" },
  ]);
  const db = await connect(core, { model, lock });
  expect(await db.spaces.list()).toEqual([
    { id: SPACE, role: "admin", model: "ours" },
    { id: OTHER, role: null, model: "another" },
  ]);
});

test("db.spaces.subscribe gives the list, then each new list the core sends", async () => {
  const { core, calls, send } = fake();
  const db = await connect(core, { model, lock });
  const seen: unknown[] = [];
  const stop = db.spaces.subscribe((all) => seen.push(all));
  await settle();
  expect(seen).toEqual([ONE]);
  expect(calls.filter(({ call }) => call === "spaces.follow")).toHaveLength(1);

  const two = [...ONE, { id: OTHER, role: "writer", model: "ours" }];
  send({ event: "spaces", spaces: two });
  expect(seen).toEqual([ONE, two]);

  stop();
  send({ event: "spaces", spaces: ONE });
  expect(seen).toEqual([ONE, two]);
});

test("a second subscriber gets the newest list at once, without asking the core again", async () => {
  const { core, calls } = fake();
  const db = await connect(core, { model, lock });
  db.spaces.subscribe(() => {});
  await settle();
  const seen: unknown[] = [];
  db.spaces.subscribe((all) => seen.push(all));
  expect(seen).toEqual([ONE]);
  expect(calls.filter(({ call }) => call === "spaces.follow")).toHaveLength(1);
});

test("a subscriber that throws does not stop the others", async () => {
  const { core, send } = fake();
  const db = await connect(core, { model, lock });
  db.spaces.subscribe(() => {
    throw new Error("the app's bug");
  });
  const seen: unknown[] = [];
  db.spaces.subscribe((all) => seen.push(all));
  await settle();
  send({ event: "spaces", spaces: [] });
  expect(seen.at(-1)).toEqual([]);
});

test("after another tab took the database over, the list is followed again", async () => {
  const { core, calls, send } = fake();
  const db = await connect(core, { model, lock });
  db.spaces.subscribe(() => {});
  await settle();
  send({ event: "reopened", sync: { connection: "none" } });
  await settle();
  expect(calls.filter(({ call }) => call === "spaces.follow")).toHaveLength(2);
});

test("space.leave asks the core to leave the space and gives its notice", async () => {
  const { core, calls } = fake();
  const db = await connect(core, { model, lock });
  const space = await db.spaces.get(SPACE);
  const left = await space.leave();
  expect(left).toEqual({ notice: LEFT });
  expect(calls.at(-1)).toEqual({ call: "space.leave", args: { space: SPACE } });
});

test("their types are RFC 0009's", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock });
  type Entry = Awaited<ReturnType<typeof db.spaces.list>>[number];
  expectTypeOf<Entry["model"]>().toEqualTypeOf<"ours" | "another">();
  expectTypeOf(db.spaces.subscribe)
    .parameter(0)
    .toEqualTypeOf<(spaces: Entry[]) => void>();
  expectTypeOf(db.spaces.subscribe).returns.toEqualTypeOf<() => void>();
  const space = await db.spaces.get(SPACE);
  expectTypeOf(space.leave).returns.toEqualTypeOf<
    Promise<{ readonly notice: Notice }>
  >();
});
