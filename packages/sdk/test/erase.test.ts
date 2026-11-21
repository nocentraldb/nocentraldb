// Erasing a person (NOC-406; RFC 0009, "The Rust SDK", "Erasing a person";
// Carsten, 2026-11-15, NOC-326): `db.erasureRequest`, `db.account.signOff`,
// `db.erase` and `space.erasures.takeBack` are the core's `erasureRequest`,
// `account.signOff`, `erase` and `erasures.takeBack`
// (crates/nocentraldb/src/bridge/erase.rs). The request and the sign-offs
// are their texts, as a pairing answer is (NOC-321). Against a fake core.
import { expect, expectTypeOf, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import {
  type Db,
  type ErasureRequest,
  type Notice,
  nc,
  type SignOffs,
} from "../src/index.ts";

const model = nc.model({
  people: nc.collection({ name: nc.string().personal({ person: "self" }) }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };
const SPACE = "0123456789abcdef0123456789abcdef";
const OTHER = "fedcba9876543210fedcba9876543210";
const ANNA = "00112233445566778899aabbccddeeff";
const SHRED = "aa".repeat(32);
const REQUEST = "ncdb-erase-1:AQID";
const SIGN_OFFS = "ncdb-sign-off-1:BAUG";
const ERASED: Notice = { code: "erased", text: "The person's ..." };
const TAKEN_BACK: Notice = { code: "takenBack", text: "The erasure ..." };

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
      const answer = (ok: unknown) => encode({ id, ok } as never);
      switch (call) {
        case "spaces.get":
          return answer(args.space);
        case "space.status":
          return answer({ role: "admin" });
        case "get":
          return answer({
            $id: ANNA,
            $space: SPACE,
            $deleted: false,
            fields: { name: "Anna" },
          });
        case "erasureRequest":
          return answer(REQUEST);
        case "account.signOff":
          return answer({ signOffs: SIGN_OFFS, notSigned: [OTHER] });
        case "erase":
          return answer({
            shreds: [{ space: SPACE, shred: SHRED, provisional: false }],
            skipped: [{ space: OTHER, reason: "noSignOff" }],
            notice: ERASED,
          });
        case "erasures.takeBack":
          return answer(TAKEN_BACK);
        default:
          return answer(null);
      }
    },
    terminate: () => {},
    listen: () => {},
  };
  return { core, calls };
};

/** A database on the fake core, and Anna as it read her. */
const anna = async () => {
  const { core, calls } = fake();
  const db = await connect(core, { model, lock });
  const space = await db.spaces.get(SPACE);
  const read = await space.get(model.people, ANNA);
  if (read === undefined) throw new Error("Anna is read");
  return { db, space, read, calls };
};

test("db.erasureRequest asks the core by the object it read and gives the request's text", async () => {
  const { db, read, calls } = await anna();
  const request = await db.erasureRequest(read);
  expect(calls.at(-1)).toEqual({
    call: "erasureRequest",
    args: { space: SPACE, collection: "people", id: ANNA },
  });
  expect(String(request)).toBe(REQUEST);
  expect(JSON.stringify({ request })).toBe(`{"request":"${REQUEST}"}`);
});

test("db.erasureRequest refuses an object the SDK did not read, before it asks the core", async () => {
  const { db, read, calls } = await anna();
  const before = calls.length;
  await expect(db.erasureRequest({ ...read })).rejects.toMatchObject({
    code: "invalidValue",
  });
  expect(calls.length).toBe(before);
});

test("nc.erasureRequest and nc.signOffs read the texts, and refuse anything else with invalidValue", () => {
  expect(String(nc.erasureRequest(` ${REQUEST}\n`))).toBe(REQUEST);
  expect(String(nc.signOffs(SIGN_OFFS))).toBe(SIGN_OFFS);
  for (const text of ["", "ncdb-pair-1:AQID", "ncdb-erase-1:", "ncdb-erase-1:a"]) {
    expect(() => nc.erasureRequest(text)).toThrow(
      expect.objectContaining({ code: "invalidValue" }),
    );
  }
  for (const text of [REQUEST, "ncdb-sign-off-1:@@@@"]) {
    expect(() => nc.signOffs(text)).toThrow(
      expect.objectContaining({ code: "invalidValue" }),
    );
  }
});

test("db.account.signOff sends the request's text and gives the sign-offs and the spaces not signed", async () => {
  const { db } = await anna();
  const { core, calls } = fake();
  const other = await connect(core, { model, lock });
  const signed = await other.account.signOff(nc.erasureRequest(REQUEST));
  expect(calls.at(-1)).toEqual({
    call: "account.signOff",
    args: { request: REQUEST },
  });
  expect(String(signed.signOffs)).toBe(SIGN_OFFS);
  expect(signed.notSigned).toEqual([OTHER]);
  await db.close();
});

test("db.erase sends both texts and gives the shreds, the skipped spaces and the notice", async () => {
  const { db, read, calls } = await anna();
  const request = await db.erasureRequest(read);
  const erased = await db.erase(request, nc.signOffs(SIGN_OFFS));
  expect(calls.at(-1)).toEqual({
    call: "erase",
    args: { request: REQUEST, signOffs: SIGN_OFFS },
  });
  expect(erased).toEqual({
    shreds: [{ space: SPACE, shred: SHRED, provisional: false }],
    skipped: [{ space: OTHER, reason: "noSignOff" }],
    notice: ERASED,
  });
});

test("space.erasures.takeBack sends the space and the shred and gives the notice", async () => {
  const { space, calls } = await anna();
  expect(await space.erasures.takeBack(SHRED)).toEqual(TAKEN_BACK);
  expect(calls.at(-1)).toEqual({
    call: "erasures.takeBack",
    args: { space: SPACE, shred: SHRED },
  });
});

test("the erasure calls have the shapes of RFC 0009's Rust calls, in camel case", () => {
  expectTypeOf<Db["erasureRequest"]>().returns.toEqualTypeOf<
    Promise<ErasureRequest>
  >();
  expectTypeOf<Db["account"]["signOff"]>().returns.toEqualTypeOf<
    Promise<{
      readonly signOffs: SignOffs;
      readonly notSigned: readonly string[];
    }>
  >();
  type Erased = Awaited<ReturnType<Db["erase"]>>;
  expectTypeOf<Erased["shreds"][number]>().toEqualTypeOf<{
    readonly space: string;
    readonly shred: string;
    readonly provisional: boolean;
  }>();
  expectTypeOf<Erased["skipped"][number]>().toEqualTypeOf<{
    readonly space: string;
    readonly reason: "noSignOff" | "notWriter" | "noKey";
  }>();
  expectTypeOf<Erased["notice"]>().toEqualTypeOf<Notice>();
});
