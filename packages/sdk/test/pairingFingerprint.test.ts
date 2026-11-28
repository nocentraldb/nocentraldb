// The fingerprint a person compares before pairing (RFC 0010, "Adding a
// device or an agent to an account", step 2; Carsten, 2026-11-27, NOC-583):
// `db.account.pairingFingerprint(invitation)` asks the core, which reads the
// invitation's text and answers `Invitation::fingerprint()`. Against a fake
// core that answers, for each of the shared vectors' texts, the digits
// `fingerprints.txt` gives; the bridge's own test checks the real core
// against the same files (crates/nocentraldb/src/bridge/tests/account.rs).

import { readFileSync } from "node:fs";
import { expect, expectTypeOf, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { type Invitation, nc } from "../src/index.ts";

const model = nc.model({
  tasks: nc.collection({ title: nc.string() }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };

const read = (file: string) =>
  readFileSync(
    new URL(`../../../docs/rfc/vectors/0003/${file}`, import.meta.url),
    "utf8",
  );
/** Each vector's fingerprint, by its name. */
const FINGERPRINTS = new Map(
  read("fingerprints.txt")
    .trim()
    .split("\n")
    .map((line) => line.split(": ") as [string, string]),
);
/** The vectors whose fingerprints the fake core knows, by their text. */
const NAMES = ["invitation-pairing", "invitation", "invitation-statement"];
const BY_TEXT = new Map(
  NAMES.map((name) => [
    read(`${name}.txt`).trim(),
    FINGERPRINTS.get(name) as string,
  ]),
);

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
      if (call !== "account.pairingFingerprint") {
        return encode({ id, ok: null });
      }
      const digits = BY_TEXT.get(args.invitation as string);
      return encode(
        digits === undefined
          ? {
              id,
              error: { code: "invalidValue", message: "not an invitation" },
            }
          : { id, ok: digits },
      );
    },
    terminate: () => {},
    listen: () => {},
  };
  return { core, calls };
};

test("pairingFingerprint sends the invitation's text and gives the core's digits", async () => {
  const { core, calls } = fake();
  const db = await connect(core, { model, lock });
  const text = read("invitation-pairing.txt").trim();
  const digits = await db.account.pairingFingerprint(nc.invitation(text));
  expect(calls.at(-1)).toEqual({
    call: "account.pairingFingerprint",
    args: { invitation: text },
  });
  expect(digits).toBe(FINGERPRINTS.get("invitation-pairing"));
  expect(digits).toMatch(/^\d{5}( \d{5}){11}$/);
});

test("pairingFingerprint gives a certified invitation's account fingerprint", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock });
  for (const name of ["invitation", "invitation-statement"]) {
    const invitation = nc.invitation(read(`${name}.txt`));
    expect(await db.account.pairingFingerprint(invitation), name).toBe(
      FINGERPRINTS.get(name),
    );
  }
});

test("pairingFingerprint fails with the core's invalidValue for a text the core does not read", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock });
  await expect(
    db.account.pairingFingerprint(nc.invitation("ncdb-invite-1:AAAA")),
  ).rejects.toMatchObject({ code: "invalidValue" });
});

test("the pairingFingerprint type is what the owner decided", async () => {
  const { core } = fake();
  const db = await connect(core, { model, lock });
  expectTypeOf(db.account.pairingFingerprint).parameters.toEqualTypeOf<
    [Invitation]
  >();
  expectTypeOf(db.account.pairingFingerprint).returns.toEqualTypeOf<
    Promise<string>
  >();
});
