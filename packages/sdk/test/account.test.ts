// Accounts, slice 1 (NOC-316; RFC 0009, "Accounts and devices"; the shapes
// the owner decided on NOC-321, Carsten, 2026-11-15): nc.open's `pairing`
// and `recover`, the recovery code until the person confirmed it, and
// `db.account.recovered()`. Against a fake core.

/** What Node's inspect shows, through the hook it calls. */
const INSPECT = Symbol.for("nodejs.util.inspect.custom");
const inspect = (value: unknown, _options?: object): string =>
  (value as Record<symbol, () => string>)[INSPECT]?.() ?? String(value);

import { expect, expectTypeOf, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { type Notice, nc, type RecoveryCode } from "../src/index.ts";

const model = nc.model({
  tasks: nc.collection({ title: nc.string() }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };
const WORDS = [
  "legal",
  "winner",
  "thank",
  "year",
  "wave",
  "sausage",
  "worth",
  "useful",
  "legal",
  "winner",
  "thank",
  "yellow",
];
const RECOVERED = {
  code: "accountRecovered",
  text: "Your account is back with a new account key. Your spaces come back when one of their members adds this device.",
};

const fake = (code: string[] | null = WORDS, recovered: unknown = null) => {
  const calls: { call: string; args: Record<string, unknown> }[] = [];
  let shown = code;
  const core: Core = {
    call: async (bytes) => {
      const { id, call, args } = decode(bytes) as {
        id: number;
        call: string;
        args: Record<string, unknown>;
      };
      calls.push({ call, args });
      switch (call) {
        case "account.recoveryCode":
          return encode({ id, ok: shown === null ? null : { words: shown } });
        case "account.confirmRecoveryCode":
          shown = null;
          return encode({ id, ok: null });
        case "account.recovered":
          return encode({ id, ok: recovered });
        default:
          return encode({ id, ok: null });
      }
    },
    terminate: () => {},
    listen: () => {},
  };
  return { core, calls };
};

test("nc.open passes pairing and recover to the core's open", async () => {
  const { core, calls } = fake();
  await connect(core, { model, lock, pairing: "request" });
  expect(calls[0]?.args).toMatchObject({ pairing: true });
  const other = fake();
  await connect(other.core, { model, lock, recover: "legal winner thank" });
  expect(other.calls[0]?.args).toMatchObject({ recover: "legal winner thank" });
});

test("recoveryCode() gives the twelve words until confirm(), then null", async () => {
  const { core, calls } = fake();
  const db = await connect(core, { model, lock });
  const code = await db.account.recoveryCode();
  expect(code?.words()).toEqual(WORDS);
  await code?.confirm();
  expect(calls.at(-1)?.call).toBe("account.confirmRecoveryCode");
  expect(await db.account.recoveryCode()).toBeNull();
});

test("a recovery code's string, JSON and inspect forms are redacted", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock });
  const code = (await db.account.recoveryCode()) as RecoveryCode;
  expect(String(code)).toBe("RecoveryCode(redacted)");
  expect(`${code}`).toBe("RecoveryCode(redacted)");
  expect(JSON.stringify({ code })).toBe('{"code":"RecoveryCode(redacted)"}');
  expect(inspect(code)).toBe("RecoveryCode(redacted)");
  for (const word of WORDS) {
    expect(JSON.stringify(code)).not.toContain(word);
    expect(inspect(code, { showHidden: true, depth: 5 })).not.toContain(word);
  }
});

test("recovered() gives accountRecovered on the open that recovered, else null", async () => {
  const plain = fake();
  const db = await connect(plain.core, { model, lock });
  expect(await db.account.recovered()).toBeNull();
  const back = fake(null, RECOVERED);
  const recovered = await connect(back.core, { model, lock, recover: "x" });
  expect(await recovered.account.recovered()).toEqual(RECOVERED);
});

test("their types are what the owner decided", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock });
  expectTypeOf(db.account.recoveryCode).returns.toEqualTypeOf<
    Promise<RecoveryCode | null>
  >();
  expectTypeOf<ReturnType<RecoveryCode["words"]>>().toEqualTypeOf<
    readonly string[]
  >();
  expectTypeOf<ReturnType<RecoveryCode["confirm"]>>().toEqualTypeOf<
    Promise<void>
  >();
  expectTypeOf(db.account.recovered).returns.toEqualTypeOf<
    Promise<Notice | null>
  >();
});
