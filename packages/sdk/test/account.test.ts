// Accounts, slice 1 (NOC-316; RFC 0009, "Accounts and devices"; the shapes
// the owner decided on NOC-321, Carsten, 2026-11-15): nc.open's `pairing`
// and `recover`, the recovery code until two of its words came back
// (NOC-502), and `db.account.recovered()`. Against a fake core.

/** What Node's inspect shows, through the hook it calls. */
const INSPECT = Symbol.for("nodejs.util.inspect.custom");
const inspect = (value: unknown, _options?: object): string =>
  (value as Record<symbol, () => string>)[INSPECT]?.() ?? String(value);

import { expect, expectTypeOf, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import {
  type Notice,
  nc,
  type PairingAnswer,
  type RecoveryCode,
} from "../src/index.ts";

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
const CHALLENGE = [4, 9];
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
          return encode({
            id,
            ok: shown === null ? null : { words: shown, challenge: CHALLENGE },
          });
        case "account.confirmRecoveryCode": {
          const answers = args.answers as { position: number; word: string }[];
          if (answers.some(({ position, word }) => word !== WORDS[position - 1])) {
            return encode({
              id,
              error: {
                code: "invalidValue",
                message: "word 9 does not match the recovery code",
              },
            });
          }
          shown = null;
          return encode({ id, ok: null });
        }
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

test("recoveryCode() gives the twelve words until two of them come back, then null", async () => {
  const { core, calls } = fake();
  const db = await connect(core, { model, lock });
  const code = (await db.account.recoveryCode()) as RecoveryCode;
  expect(code.words()).toEqual(WORDS);
  expect(code.challenge()).toEqual([4, 9]);
  await code.confirm(["Wave ", "legal"]);
  expect(calls.at(-1)).toEqual({
    call: "account.confirmRecoveryCode",
    args: {
      answers: [
        { position: 4, word: "Wave " },
        { position: 9, word: "legal" },
      ],
    },
  });
  expect(await db.account.recoveryCode()).toBeNull();
});

test("a wrong word fails with invalidValue and the code stays", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock });
  const code = (await db.account.recoveryCode()) as RecoveryCode;
  await expect(code.confirm(["wave", "zoo"])).rejects.toMatchObject({
    code: "invalidValue",
  });
  expect((await db.account.recoveryCode())?.words()).toEqual(WORDS);
  await code.confirm(["wave", "legal"]);
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
  expectTypeOf<ReturnType<RecoveryCode["challenge"]>>().toEqualTypeOf<
    readonly [number, number]
  >();
  expectTypeOf<Parameters<RecoveryCode["confirm"]>>().toEqualTypeOf<
    [answers: readonly [string, string]]
  >();
  expectTypeOf<ReturnType<RecoveryCode["confirm"]>>().toEqualTypeOf<
    Promise<void>
  >();
  expectTypeOf(db.account.recovered).returns.toEqualTypeOf<
    Promise<Notice | null>
  >();
});

// Slice 2: pairing (NOC-321 points 3 to 5).
const ANSWER = "ncdb-pair-1:AAECAwQFBgc";
const SPACE = "0123456789abcdef0123456789abcdef";
const ADDED = {
  code: "deviceAdded",
  text: "Your new device was added to this space and reads its whole history.",
};
const PAIRED = {
  code: "devicePaired",
  text: "This device now holds your account key. Keep your recovery code: it is the way back if you lose every device.",
};

const pairing = () => {
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
        case "account.pair":
          return encode({
            id,
            ok: {
              answer: ANSWER,
              added: [{ space: SPACE, notice: ADDED }],
              askAdmins: [],
            },
          });
        case "account.completePairing":
          return encode({ id, ok: { notices: [PAIRED] } });
        default:
          return encode({ id, ok: null });
      }
    },
    terminate: () => {},
    listen: () => {},
  };
  return { core, calls };
};

test("pair sends the kind and the invitation's text, and gives the answer, the spaces added and the admins to ask", async () => {
  const { core, calls } = pairing();
  const db = await connect(core, { model, lock });
  const invitation = nc.invitation("ncdb-invite-1:AAECAwQ");
  const paired = await db.account.pair({ kind: "device", invitation });
  expect(calls.at(-1)).toEqual({
    call: "account.pair",
    args: { kind: "device", invitation: "ncdb-invite-1:AAECAwQ" },
  });
  expect(String(paired.answer)).toBe(ANSWER);
  expect(paired.added).toEqual([{ space: SPACE, notice: ADDED }]);
  expect(paired.askAdmins).toEqual([]);
});

test("completePairing sends the answer's text and gives the notices", async () => {
  const { core, calls } = pairing();
  const db = await connect(core, { model, lock, pairing: "request" });
  const result = await db.account.completePairing(nc.pairingAnswer(ANSWER));
  expect(calls.at(-1)).toEqual({
    call: "account.completePairing",
    args: { answer: ANSWER },
  });
  expect(result).toEqual({ notices: [PAIRED] });
});

test("nc.pairingAnswer reads an answer's text back, and refuses a text that is not one", () => {
  expect(String(nc.pairingAnswer(`  ${ANSWER}\n`))).toBe(ANSWER);
  for (const text of [
    "",
    "ncdb-invite-1:AAAA",
    "ncdb-pair-1:",
    "ncdb-pair-1:A",
    "ncdb-pair-1:AA=A",
  ]) {
    expect(() => nc.pairingAnswer(text), text).toThrow(
      expect.objectContaining({ code: "invalidValue" }),
    );
  }
});

test("the pairing types are what the owner decided", async () => {
  const { core } = pairing();
  const db = await connect(core, { model, lock });
  expectTypeOf(nc.pairingAnswer).returns.toEqualTypeOf<PairingAnswer>();
  expectTypeOf<PairingAnswer["toString"]>().returns.toEqualTypeOf<string>();
  type Paired = Awaited<ReturnType<typeof db.account.pair>>;
  expectTypeOf<Paired["answer"]>().toEqualTypeOf<PairingAnswer>();
  expectTypeOf<Paired["added"]>().toEqualTypeOf<
    readonly { readonly space: string; readonly notice: Notice }[]
  >();
  expectTypeOf<Paired["askAdmins"]>().toEqualTypeOf<readonly string[]>();
  expectTypeOf(db.account.completePairing).returns.toEqualTypeOf<
    Promise<{ readonly notices: readonly Notice[] }>
  >();
});

// Slice 3: removing a lost device (NOC-321 point 6).
const CODE_WORDS = WORDS.join(" ");
const MEMBER = "ab".repeat(32);
const REMOVED = {
  code: "memberRemoved",
  text: "The removed member keeps its copy of everything it received before now.",
};
const RENEWED = {
  code: "accountKeyRenewed",
  text: "Your account has a new account key. Members that have not learnt it yet still accept the old one.",
};

const removing = () => {
  const calls: { call: string; args: Record<string, unknown> }[] = [];
  const core: Core = {
    call: async (bytes) => {
      const { id, call, args } = decode(bytes) as {
        id: number;
        call: string;
        args: Record<string, unknown>;
      };
      calls.push({ call, args });
      return call === "account.removeMember"
        ? encode({
            id,
            ok: {
              removed: [{ space: SPACE, notice: REMOVED }],
              askAdmins: [],
              notice: RENEWED,
            },
          })
        : encode({ id, ok: null });
    },
    terminate: () => {},
    listen: () => {},
  };
  return { core, calls };
};

test("removeMember sends the member and the words, and gives the spaces it was removed from, the admins to ask and the notice", async () => {
  const { core, calls } = removing();
  const db = await connect(core, { model, lock });
  const removed = await db.account.removeMember(MEMBER, {
    recover: CODE_WORDS,
  });
  expect(calls.at(-1)).toEqual({
    call: "account.removeMember",
    args: { member: MEMBER, recover: CODE_WORDS },
  });
  expect(removed).toEqual({
    removed: [{ space: SPACE, notice: REMOVED }],
    askAdmins: [],
    notice: RENEWED,
  });
});

test("the removeMember types are what the owner decided", async () => {
  const { core } = removing();
  const db = await connect(core, { model, lock });
  expectTypeOf(db.account.removeMember).parameters.toEqualTypeOf<
    [string, { recover: string }]
  >();
  expectTypeOf(db.account.removeMember).returns.toEqualTypeOf<
    Promise<{
      readonly removed: readonly {
        readonly space: string;
        readonly notice: Notice;
      }[];
      readonly askAdmins: readonly string[];
      readonly notice: Notice;
    }>
  >();
});
