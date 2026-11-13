// db.changes (RFC 0009, "Subscribe"; NOC-176 slice 3): after each fold,
// the space, the collection and the ids it touched, and whether the change
// was this member's own write ("local") or one sync brought ("remote").
// Ids only, never values. Against a fake core that sends the bridge's
// folded events (crates/nocentraldb/src/bridge/mod.rs).
import { expect, expectTypeOf, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { nc } from "../src/index.ts";

const model = nc.model({
  tasks: nc.collection({ title: nc.string() }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };
const SPACE = "0123456789abcdef0123456789abcdef";
const A = "aa".repeat(16);
const B = "bb".repeat(16);

type Change = {
  space: string;
  collection: string;
  ids: readonly string[];
  origin: "local" | "remote";
};

const fake = () => {
  let listener: ((event: Uint8Array) => void) | undefined;
  const core: Core = {
    call: async (bytes) => {
      const { id } = decode(bytes) as { id: number };
      return encode({ id, ok: null });
    },
    terminate: () => {},
    listen: (given) => {
      listener = given;
    },
  };
  const fold = (changes: Change[]) =>
    listener?.(
      encode({
        event: "folded",
        folds: [
          {
            space: SPACE,
            collections: ["tasks"],
            changes: changes.map(({ collection, ids, origin }) => ({
              collection,
              ids,
              origin,
            })),
          },
        ],
      }),
    );
  return { core, fold };
};

test("db.changes.subscribe gives the space, collection, ids and origin of each fold", async () => {
  const { core, fold } = fake();
  const db = await connect(core, { model, lock });
  const seen: Change[] = [];
  db.changes.subscribe((change) => seen.push(change));
  fold([{ space: SPACE, collection: "tasks", ids: [A, B], origin: "local" }]);
  fold([{ space: SPACE, collection: "tasks", ids: [B], origin: "remote" }]);
  expect(seen).toEqual([
    { space: SPACE, collection: "tasks", ids: [A, B], origin: "local" },
    { space: SPACE, collection: "tasks", ids: [B], origin: "remote" },
  ]);
});

test("its listener is typed by RFC 0009's fields and origins", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock });
  db.changes.subscribe((change) => {
    expectTypeOf(change.space).toEqualTypeOf<string>();
    expectTypeOf(change.collection).toEqualTypeOf<string>();
    expectTypeOf(change.ids).toEqualTypeOf<readonly string[]>();
    expectTypeOf(change.origin).toEqualTypeOf<"local" | "remote">();
  });
});

test("a fold that touched no object calls no listener", async () => {
  const { core, fold } = fake();
  const db = await connect(core, { model, lock });
  const seen: unknown[] = [];
  db.changes.subscribe((change) => seen.push(change));
  fold([]);
  expect(seen).toEqual([]);
});

test("the function it returns stops it, and so does closing the database", async () => {
  const { core, fold } = fake();
  const db = await connect(core, { model, lock });
  const first: unknown[] = [];
  const second: unknown[] = [];
  const stop = db.changes.subscribe((change) => first.push(change));
  db.changes.subscribe((change) => second.push(change));
  stop();
  fold([{ space: SPACE, collection: "tasks", ids: [A], origin: "local" }]);
  expect(first).toEqual([]);
  expect(second.length).toBe(1);
  await db.close();
  fold([{ space: SPACE, collection: "tasks", ids: [A], origin: "local" }]);
  expect(second.length).toBe(1);
});

test("a listener that throws does not stop the others", async () => {
  const { core, fold } = fake();
  const db = await connect(core, { model, lock });
  const seen: unknown[] = [];
  db.changes.subscribe(() => {
    throw new Error("app bug");
  });
  db.changes.subscribe((change) => seen.push(change));
  fold([{ space: SPACE, collection: "tasks", ids: [A], origin: "local" }]);
  expect(seen.length).toBe(1);
});
