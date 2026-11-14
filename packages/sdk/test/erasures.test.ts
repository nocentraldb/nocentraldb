// The audit of a space's erasures (NOC-315, slice 2; RFC 0009, "Erasures";
// Carsten, 2026-11-12, NOC-246): `space.erasures.list()` is the core's
// `erasures.list` (crates/nocentraldb/src/bridge/erasures.rs). Against a
// fake core.
import { expect, expectTypeOf, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { nc, type Space } from "../src/index.ts";

// RFC 0009 names no TypeScript type for an erasure; it is what the call gives.
type Erasure = Awaited<ReturnType<Space["erasures"]["list"]>>[number];

const model = nc.model({
  tasks: nc.collection({ title: nc.string() }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };
const SPACE = "0123456789abcdef0123456789abcdef";
const ACCOUNT =
  "12345 67890 12345 67890 12345 67890 12345 67890 12345 67890 12345 67890";

const ERASURE = {
  shred: "aa".repeat(32),
  state: "provisional",
  performer: { id: "11".repeat(32), account: ACCOUNT },
  signOff: ACCOUNT,
  members: [
    {
      id: "11".repeat(32),
      account: ACCOUNT,
      current: true,
      acknowledged: "deleted",
    },
    {
      id: "22".repeat(32),
      account: ACCOUNT,
      current: false,
      acknowledged: "missing",
    },
  ],
};

const fake = () => {
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
        case "spaces.get":
          return encode({ id, ok: args.space });
        case "space.status":
          return encode({ id, ok: { role: "reader" } });
        case "erasures.list":
          return encode({ id, ok: [ERASURE] });
        default:
          return encode({ id, ok: null });
      }
    },
    terminate: () => {},
    listen: () => {},
  };
  return { core, calls };
};

test("space.erasures.list asks the core for the space's erasures and gives them", async () => {
  const { core, calls } = fake();
  const db = await connect(core, { model, lock });
  const space = await db.spaces.get(SPACE);
  expect(await space.erasures.list()).toEqual([ERASURE]);
  expect(calls.at(-1)).toEqual({
    call: "erasures.list",
    args: { space: SPACE },
  });
});

test("an erasure's type is RFC 0009's", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock });
  const space = await db.spaces.get(SPACE);
  expectTypeOf(space.erasures.list).returns.toEqualTypeOf<Promise<Erasure[]>>();
  expectTypeOf<Erasure["shred"]>().toEqualTypeOf<string>();
  expectTypeOf<Erasure["state"]>().toEqualTypeOf<
    "final" | "provisional" | "takenBack"
  >();
  expectTypeOf<Erasure["performer"]>().toEqualTypeOf<{
    readonly id: string;
    readonly account: string;
  }>();
  expectTypeOf<Erasure["signOff"]>().toEqualTypeOf<string>();
  expectTypeOf<Erasure["members"][number]["acknowledged"]>().toEqualTypeOf<
    "deleted" | "seen" | "missing"
  >();
  expectTypeOf<
    Erasure["members"][number]["current"]
  >().toEqualTypeOf<boolean>();
});
