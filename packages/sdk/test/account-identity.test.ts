// `db.account.identity` (RFC 0015, "In the SDK"; names as pm built them on
// NOC-469, waiting on the owner's NOC-493): the account's recovery key in
// hexadecimal, public, which the core answers on open, or null while the
// database waits to be paired; completing a pairing gives it. Against a
// fake core.

import { expect, expectTypeOf, test } from "vitest";
import { decode, encode } from "../src/cbor.ts";
import { type Core, connect } from "../src/connect.ts";
import { nc, pairingAnswer } from "../src/index.ts";

const model = nc.model({
  tasks: nc.collection({ title: nc.string() }),
});
const lock = { model: { id: "00112233445566778899aabbccddeeff" } };
const IDENTITY = "90e506d6c805585c4901c7b7cd36405df43b0cb4678fc9afb7378af4d438d743";

const fake = (identity: string | null) => {
  const core: Core = {
    call: async (bytes) => {
      const { id, call } = decode(bytes) as { id: number; call: string };
      switch (call) {
        case "open":
          return encode({ id, ok: { identity } });
        case "account.completePairing":
          return encode({ id, ok: { notices: [], identity: IDENTITY } });
        default:
          return encode({ id, ok: null });
      }
    },
    terminate: () => {},
    listen: () => {},
  };
  return core;
};

test("db.account.identity is the identity the core answers on open", async () => {
  const db = await connect(fake(IDENTITY), { model, lock });
  expect(db.account.identity).toBe(IDENTITY);
  expectTypeOf(db.account.identity).toEqualTypeOf<string | null>();
});

test("a database waiting to be paired has none until the pairing completes", async () => {
  const db = await connect(fake(null), { model, lock, pairing: "request" });
  expect(db.account.identity).toBeNull();
  await db.account.completePairing(pairingAnswer("ncdb-pair-1:AQ"));
  expect(db.account.identity).toBe(IDENTITY);
});
